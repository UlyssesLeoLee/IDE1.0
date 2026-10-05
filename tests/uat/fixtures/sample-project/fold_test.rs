fn outer() {
    fn inner() {
        x();
    }
    inner();
}
fn other() {
    y();
}
