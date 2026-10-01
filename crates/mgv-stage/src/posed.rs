//! Meshes posed on the CPU, in world space: what overlays draw (wireframe,
//! normals, walkmeshes) and what picking casts rays against. The posing
//! follows the renderer's (`mg-render`'s bone matrices: bind pose to current
//! pose in the skin node's space; animated and dangly vertices from the
//! actor's mesh state), so the overlay sits on what is drawn.

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use mg_mdl::{Mesh, MeshExtra, Model, NodeKind};

use crate::{Actor, ActorId, Stage};

/// Whose mesh it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Actor(ActorId),
    /// A walkmesh of [`Stage::walkmeshes`].
    Walkmesh(usize),
}

/// A mesh in world space.
#[derive(Debug, Clone, PartialEq)]
pub struct Posed {
    pub owner: Owner,
    /// The mesh's node in its model.
    pub node: usize,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub faces: Vec<[u32; 3]>,
    /// Per face: the surface material (walkmeshes).
    pub materials: Vec<u32>,
    /// A collision mesh (aabb), not drawn by the renderer.
    pub walkmesh: bool,
}

/// A walkmesh shown with a model: its own nodes (in their rest pose), placed
/// as the host actor is, or in the world.
#[derive(Debug, Clone)]
pub struct Walkmesh {
    pub name: String,
    pub model: Arc<Model>,
    pub host: Option<ActorId>,
}

/// The inverse bind pose per bone of a skin, as the renderer makes it.
fn inverse_binds(model: &Model, rest: &[Mat4], node: usize, mesh: &Mesh) -> Vec<Mat4> {
    let MeshExtra::Skin(s) = &mesh.extra else { return Vec::new() };
    let stored = s.inverse_bind.len() == model.nodes.len();
    s.bones
        .iter()
        .map(|&b| {
            if stored {
                let (q, t) = s.inverse_bind[b];
                let q = Quat::from_array(q);
                let q = if q.length_squared() > 1e-12 { q.normalize() } else { Quat::IDENTITY };
                Mat4::from_rotation_translation(q, Vec3::from(t))
            } else {
                rest.get(b).copied().unwrap_or(Mat4::IDENTITY).inverse()
                    * rest.get(node).copied().unwrap_or(Mat4::IDENTITY)
            }
        })
        .collect()
}

/// One mesh of a model at a pose, in world space.
#[allow(clippy::too_many_arguments)]
fn pose_mesh(
    model: &Model,
    rest: &[Mat4],
    pose: &[Mat4],
    world: Mat4,
    node: usize,
    mesh: &Mesh,
    replaced: Option<&[mg_render::model::Vertex]>,
    owner: Owner,
) -> Posed {
    let node_now = pose.get(node).copied().unwrap_or(Mat4::IDENTITY);
    let to_world = world * node_now;
    let local: Vec<(Vec3, Vec3)> = match replaced {
        Some(v) => v.iter().map(|v| (Vec3::from(v.pos), Vec3::from(v.normal))).collect(),
        None => mesh
            .vertices
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    Vec3::from(*p),
                    Vec3::from(mesh.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0])),
                )
            })
            .collect(),
    };
    let skinned: Vec<(Vec3, Vec3)> = match &mesh.extra {
        MeshExtra::Skin(s) if s.weights.len() == local.len() && !s.bones.is_empty() => {
            let inv = inverse_binds(model, rest, node, mesh);
            let to_skin = node_now.inverse();
            let bones: Vec<Mat4> = s
                .bones
                .iter()
                .zip(&inv)
                .map(|(&b, ib)| to_skin * pose.get(b).copied().unwrap_or(Mat4::IDENTITY) * *ib)
                .collect();
            local
                .iter()
                .zip(&s.weights)
                .map(|(&(p, n), w)| {
                    let total: f32 = w.iter().map(|(_, x)| x).sum();
                    if total <= 0.0 {
                        return (p, n);
                    }
                    let (mut sp, mut sn) = (Vec3::ZERO, Vec3::ZERO);
                    for &(b, x) in w.iter().filter(|(_, x)| *x > 0.0) {
                        let m = bones.get(usize::from(b)).copied().unwrap_or(Mat4::IDENTITY);
                        sp += m.transform_point3(p) * x;
                        sn += m.transform_vector3(n) * x;
                    }
                    (sp, sn)
                })
                .collect()
        }
        _ => local,
    };
    Posed {
        owner,
        node,
        positions: skinned.iter().map(|(p, _)| to_world.transform_point3(*p)).collect(),
        normals: skinned
            .iter()
            .map(|(_, n)| to_world.transform_vector3(*n).normalize_or_zero())
            .collect(),
        faces: mesh.faces.iter().map(|f| f.vertices).collect(),
        materials: mesh.faces.iter().map(|f| f.material).collect(),
        walkmesh: matches!(mesh.extra, MeshExtra::Aabb(_)),
    }
}

impl Stage {
    /// Every mesh of the visible actors as last stepped, drawn or not
    /// (`all`: also those the renderer skips: `render 0` and walkmeshes).
    pub fn posed(&self, all: bool) -> Vec<Posed> {
        let mut out = Vec::new();
        for (i, a) in self.actors.iter().enumerate().filter(|(_, a)| a.visible) {
            out.extend(posed_actor(ActorId(i), a, all));
        }
        out
    }

    /// The walkmeshes shown with the stage's models.
    pub fn posed_walkmeshes(&self) -> Vec<Posed> {
        let mut out = Vec::new();
        for (i, w) in self.walkmeshes.iter().enumerate() {
            let world = w.host.and_then(|h| self.actor(h)).map_or(Mat4::IDENTITY, Actor::world);
            let rest = mg_render::rest_pose(&w.model);
            for (n, node) in w.model.nodes.iter().enumerate() {
                let NodeKind::Mesh(m) = &node.kind else { continue };
                if m.faces.is_empty() {
                    continue;
                }
                let mut p =
                    pose_mesh(&w.model, &rest, &rest, world, n, m, None, Owner::Walkmesh(i));
                p.walkmesh = true;
                out.push(p);
            }
        }
        out
    }
}

fn posed_actor(id: ActorId, a: &Actor, all: bool) -> Vec<Posed> {
    let gm = &a.model;
    let model = &gm.model;
    let mut out = Vec::new();
    for (n, node) in model.nodes.iter().enumerate() {
        let NodeKind::Mesh(m) = &node.kind else { continue };
        let drawn = gm.meshes.iter().position(|g| g.node == n);
        if m.faces.is_empty() || (drawn.is_none() && !all) {
            continue;
        }
        let replaced =
            drawn.and_then(|j| a.state.meshes.get(j)).and_then(|o| o.vertices.as_deref());
        out.push(pose_mesh(model, &gm.rest, &a.pose, a.world, n, m, replaced, Owner::Actor(id)));
    }
    out
}

/// Where a ray first meets a triangle (Möller–Trumbore, both sides): the
/// index into `meshes` and the distance along `dir` (normalised).
pub fn ray_hit(meshes: &[Posed], origin: Vec3, dir: Vec3) -> Option<(usize, f32)> {
    let dir = dir.normalize_or_zero();
    let mut best: Option<(usize, f32)> = None;
    for (mi, m) in meshes.iter().enumerate() {
        for f in &m.faces {
            let [Some(a), Some(b), Some(c)] = f.map(|i| m.positions.get(i as usize).copied())
            else {
                continue;
            };
            let (e1, e2) = (b - a, c - a);
            let p = dir.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-12 {
                continue;
            }
            let inv = 1.0 / det;
            let s = origin - a;
            let u = s.dot(p) * inv;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = s.cross(e1);
            let v = dir.dot(q) * inv;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let t = e2.dot(q) * inv;
            if t > 1e-5 && best.is_none_or(|(_, bt)| t < bt) {
                best = Some((mi, t));
            }
        }
    }
    best
}

/// The walkmesh files that go with a model, by its name (tiles `.wok`,
/// placeables `.pwk`, doors `.dwk`), first found: its nodes under the
/// object's root.
pub fn walkmesh_for(
    lib: &mgv_library::Library,
    name: &str,
) -> Option<(mg_core::ResType, Arc<Model>)> {
    use mg_core::ResType;
    use mg_mdl::walkmesh::{Walkmesh, WalkmeshKind};
    for (t, kind) in [
        (ResType::WOK, WalkmeshKind::Tile),
        (ResType::PWK, WalkmeshKind::Placeable),
        (ResType::DWK, WalkmeshKind::Door),
    ] {
        let Some(key) = mg_resman::ResKey::parse(name, t) else { continue };
        if let Some(data) = lib.get(&key)
            && let Ok(w) = Walkmesh::read(&data, kind)
        {
            return Some((t, Arc::new(w.model)));
        }
    }
    None
}

/// `surfacemat.2da`: each material's label and whether creatures walk on
/// it.
pub fn surface_materials(lib: &mgv_library::Library) -> Vec<(String, bool)> {
    let Ok(t) = lib.game().table("surfacemat") else { return Vec::new() };
    (0..t.len())
        .map(|r| {
            let label = t.get(r, "Label").unwrap_or("?").to_string();
            let walk = t.get(r, "Walk").and_then(mg_2da::parse_int).is_some_and(|v| v != 0);
            (label, walk)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri(z: f32) -> Posed {
        Posed {
            owner: Owner::Walkmesh(0),
            node: 0,
            positions: vec![
                Vec3::new(-1.0, -1.0, z),
                Vec3::new(1.0, -1.0, z),
                Vec3::new(0.0, 1.0, z),
            ],
            normals: vec![Vec3::Z; 3],
            faces: vec![[0, 1, 2]],
            materials: vec![1],
            walkmesh: true,
        }
    }

    #[test]
    fn rays_find_the_nearest_face() {
        let meshes = [tri(0.0), tri(2.0)];
        let hit = ray_hit(&meshes, Vec3::new(0.0, 0.0, 10.0), -Vec3::Z).unwrap();
        assert_eq!(hit.0, 1);
        assert!((hit.1 - 8.0).abs() < 1e-5);
        assert!(ray_hit(&meshes, Vec3::new(5.0, 5.0, 10.0), -Vec3::Z).is_none());
        assert!(ray_hit(&meshes, Vec3::new(0.0, 0.0, -10.0), -Vec3::Z).is_none(), "behind");
    }
}
