use zbus::{blocking::Connection, proxy};

const APP_ID: &str = "claude-plasmoid";
const FOLDER: &str = "claude-plasmoid";

#[proxy(
    interface = "org.kde.KWallet",
    default_service = "org.kde.kwalletd6",
    default_path = "/modules/kwalletd6"
)]
trait KWallet {
    #[zbus(name = "networkWallet")]
    fn network_wallet(&self) -> zbus::Result<String>;

    #[zbus(name = "open")]
    fn open(&self, wallet: &str, wid: i64, appid: &str) -> zbus::Result<i32>;

    #[zbus(name = "close")]
    fn close(&self, handle: i32, force: bool, appid: &str) -> zbus::Result<i32>;

    #[zbus(name = "readPassword")]
    fn read_password(
        &self,
        handle: i32,
        folder: &str,
        key: &str,
        appid: &str,
    ) -> zbus::Result<String>;

    #[zbus(name = "writePassword")]
    fn write_password(
        &self,
        handle: i32,
        folder: &str,
        key: &str,
        value: &str,
        appid: &str,
    ) -> zbus::Result<i32>;

    #[zbus(name = "hasEntry")]
    fn has_entry(&self, handle: i32, folder: &str, key: &str, appid: &str) -> zbus::Result<bool>;

    #[zbus(name = "removeEntry")]
    fn remove_entry(&self, handle: i32, folder: &str, key: &str, appid: &str) -> zbus::Result<i32>;
}

pub struct Credentials {
    pub url: String,
    pub username: String,
    pub password: String,
}

pub fn read_credentials() -> Result<Credentials, String> {
    let conn = Connection::session().map_err(|e| e.to_string())?;
    let proxy = KWalletProxyBlocking::new(&conn).map_err(|e| e.to_string())?;

    let wallet = proxy.network_wallet().map_err(|e| e.to_string())?;
    // wid=0: plasmoids don't have a meaningful X11/Wayland window ID. kwalletd falls back to
    // centering the unlock dialog on screen, which is acceptable.
    let handle = proxy.open(&wallet, 0, APP_ID).map_err(|e| e.to_string())?;
    if handle < 0 {
        return Err("KWallet is locked or unavailable".to_string());
    }

    let url = proxy
        .read_password(handle, FOLDER, "url", APP_ID)
        .map_err(|e| e.to_string())?;
    let username = proxy
        .read_password(handle, FOLDER, "username", APP_ID)
        .map_err(|e| e.to_string())?;
    let password = proxy
        .read_password(handle, FOLDER, "password", APP_ID)
        .map_err(|e| e.to_string())?;

    let _ = proxy.close(handle, false, APP_ID);

    if url.is_empty() || username.is_empty() {
        return Err("No credentials stored — configure in the applet".to_string());
    }

    Ok(Credentials {
        url,
        username,
        password,
    })
}

pub fn write_credentials(url: &str, username: &str, password: &str) -> Result<(), String> {
    let conn = Connection::session().map_err(|e| e.to_string())?;
    let proxy = KWalletProxyBlocking::new(&conn).map_err(|e| e.to_string())?;

    let wallet = proxy.network_wallet().map_err(|e| e.to_string())?;
    let handle = proxy.open(&wallet, 0, APP_ID).map_err(|e| e.to_string())?;
    if handle < 0 {
        return Err("KWallet is locked or unavailable".to_string());
    }

    proxy
        .write_password(handle, FOLDER, "url", url, APP_ID)
        .map_err(|e| e.to_string())?;
    proxy
        .write_password(handle, FOLDER, "username", username, APP_ID)
        .map_err(|e| e.to_string())?;
    proxy
        .write_password(handle, FOLDER, "password", password, APP_ID)
        .map_err(|e| e.to_string())?;

    let _ = proxy.close(handle, false, APP_ID);
    Ok(())
}

pub fn delete_credentials() -> Result<(), String> {
    let conn = Connection::session().map_err(|e| e.to_string())?;
    let proxy = KWalletProxyBlocking::new(&conn).map_err(|e| e.to_string())?;

    let wallet = proxy.network_wallet().map_err(|e| e.to_string())?;
    let handle = proxy.open(&wallet, 0, APP_ID).map_err(|e| e.to_string())?;
    if handle < 0 {
        return Err("KWallet is locked or unavailable".to_string());
    }

    let _ = proxy.remove_entry(handle, FOLDER, "url", APP_ID);
    let _ = proxy.remove_entry(handle, FOLDER, "username", APP_ID);
    let _ = proxy.remove_entry(handle, FOLDER, "password", APP_ID);

    let _ = proxy.close(handle, false, APP_ID);
    Ok(())
}
