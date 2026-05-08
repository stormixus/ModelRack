pub const INTER_VARIABLE: &[u8] = include_bytes!("../assets/fonts/InterVariable.ttf");
pub const PRETENDARD_VARIABLE: &[u8] = include_bytes!("../assets/fonts/PretendardVariable.ttf");
pub const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");

pub const INTER_FAMILY: &str = "Inter";
pub const PRETENDARD_FAMILY: &str = "Pretendard";
pub const MONO_FAMILY: &str = "JetBrains Mono";

pub fn install_slint_fonts() {
    use std::sync::Arc;

    use slint::fontique_08::{fontique, shared_collection};

    let mut collection = shared_collection();
    let _inter =
        collection.register_fonts(fontique::Blob::new(Arc::new(INTER_VARIABLE.to_vec())), None);
    let pretendard = collection.register_fonts(
        fontique::Blob::new(Arc::new(PRETENDARD_VARIABLE.to_vec())),
        None,
    );
    let _mono = collection.register_fonts(
        fontique::Blob::new(Arc::new(JETBRAINS_MONO_REGULAR.to_vec())),
        None,
    );

    let hangul = fontique::FallbackKey::new(fontique::Script::from_str_unchecked("Hang"), None);
    collection.append_fallbacks(hangul, pretendard.iter().map(|font| font.0));
}
