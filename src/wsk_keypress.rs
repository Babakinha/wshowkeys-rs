pub struct WskKeypress {
    pub sym: xkbcommon::xkb::Keysym, // ? Is this right
    pub name: String,
    pub utf8: String,
}