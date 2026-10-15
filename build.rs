// Embute o icone no GameRec.exe (precisa do rc.exe/windres; sem eles o build segue sem icone).
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set("ProductName", "GameRec");
    res.set("FileDescription", "GameRec - gravador de gameplay");
    if let Err(e) = res.compile() {
        println!("cargo:warning=icone nao embutido no exe: {e}");
    }
}
