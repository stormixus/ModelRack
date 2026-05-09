fn main() {
    // Slint 1.16 keeps DragArea/DropArea behind the experimental registry.
    // ModelRack uses them only for internal model-card -> sidebar-tag assignment.
    unsafe { std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1") };
    slint_build::compile("ui/modelrack.slint").expect("Slint build failed");
}
