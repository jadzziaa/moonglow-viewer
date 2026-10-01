//! What a file is: by its extension, checked against its content, so a
//! misnamed file still opens as what it is.

use mg_core::ResType;

/// What the viewer does with a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A model (MDL, binary or ASCII).
    Model,
    /// A walkmesh (WOK, PWK, DWK): ASCII-model shaped.
    Walkmesh,
    /// An image (TGA, DDS).
    Texture,
    /// A layered, palette-coloured texture.
    Plt,
    /// Texture settings.
    Txi,
    /// An EE material.
    Material,
    /// A creature, item, placeable or door blueprint.
    Blueprint,
    /// A 2DA table.
    TwoDa,
    /// A tileset.
    Tileset,
    /// A hak, module or ERF.
    Archive,
    /// Anything else.
    Other,
}

impl Kind {
    /// The kind of a resource type.
    pub fn of(restype: ResType) -> Kind {
        match restype {
            ResType::MDL => Kind::Model,
            ResType::WOK | ResType::PWK | ResType::DWK => Kind::Walkmesh,
            ResType::TGA | ResType::DDS => Kind::Texture,
            ResType::PLT => Kind::Plt,
            ResType::TXI => Kind::Txi,
            ResType::MTR => Kind::Material,
            ResType::UTC | ResType::UTI | ResType::UTP | ResType::UTD => Kind::Blueprint,
            ResType::TWODA => Kind::TwoDa,
            ResType::SET => Kind::Tileset,
            ResType::HAK | ResType::MOD | ResType::ERF | ResType::NWM | ResType::SAV => {
                Kind::Archive
            }
            _ => Kind::Other,
        }
    }

    /// Whether the viewer shows it in 3D.
    pub fn is_3d(self) -> bool {
        matches!(self, Kind::Model | Kind::Walkmesh | Kind::Blueprint | Kind::Material)
    }
}

/// The kind of a file from its extension (without the dot) and content.
/// Content wins where it is unambiguous (archives, binary models, DDS,
/// PLT); otherwise the extension decides.
pub fn detect(ext: Option<&str>, data: &[u8]) -> Kind {
    let by_ext = ext.and_then(ResType::from_extension).map_or(Kind::Other, Kind::of);
    let head = data.get(..8).unwrap_or(data);
    if head.starts_with(b"DDS ") {
        return Kind::Texture;
    }
    if head.starts_with(b"PLT V1") {
        return Kind::Plt;
    }
    for sig in [&b"HAK V1"[..], b"MOD V1", b"ERF V1", b"HAK E1", b"MOD E1", b"ERF E1", b"NWM V1"] {
        if head.starts_with(sig) {
            return Kind::Archive;
        }
    }
    if mg_mdl::is_binary(data) && by_ext != Kind::Texture {
        return Kind::Model;
    }
    if by_ext == Kind::Other && looks_like_ascii_model(data) {
        return Kind::Model;
    }
    by_ext
}

/// Whether text starts like an ASCII model (`newmodel`, `beginmodelgeom` or
/// a walkmesh's `beginwalkmeshgeom` before any node).
fn looks_like_ascii_model(data: &[u8]) -> bool {
    let text = String::from_utf8_lossy(&data[..data.len().min(4096)]);
    text.lines()
        .map(|l| l.split('#').next().unwrap_or_default().trim().to_ascii_lowercase())
        .filter(|l| !l.is_empty())
        .take(20)
        .any(|l| {
            l.starts_with("newmodel ")
                || l.starts_with("beginmodelgeom ")
                || l.starts_with("beginwalkmeshgeom ")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_and_content() {
        assert_eq!(detect(Some("mdl"), b"newmodel x\n"), Kind::Model);
        assert_eq!(detect(Some("MDL"), &[0; 16]), Kind::Model);
        assert_eq!(detect(Some("wok"), b"#MAXWALKMESH ASCII\n"), Kind::Walkmesh);
        assert_eq!(detect(Some("tga"), &[0; 16]), Kind::Texture, "a TGA may start with zeros");
        assert_eq!(detect(Some("tga"), b"DDS |....."), Kind::Texture);
        assert_eq!(detect(Some("bin"), b"PLT V1  rest"), Kind::Plt);
        assert_eq!(detect(Some("mod"), b"MOD V1.0rest"), Kind::Archive);
        assert_eq!(detect(Some("txt"), b"# comment\nnewmodel foo\n"), Kind::Model);
        assert_eq!(detect(None, b"hello"), Kind::Other);
        assert_eq!(detect(Some("utc"), b"UTC V3.2"), Kind::Blueprint);
        assert_eq!(detect(Some("mtr"), b"texture0 x"), Kind::Material);
        assert_eq!(detect(Some("2da"), b"2DA V2.0"), Kind::TwoDa);
    }

    #[test]
    fn short_and_empty_input() {
        assert_eq!(detect(Some("mdl"), b""), Kind::Model);
        assert_eq!(detect(None, b""), Kind::Other);
        assert_eq!(detect(None, &[0; 3]), Kind::Other);
    }
}
