use rcon::{AsyncStdStream, Connection, Error};

pub fn create_connection(address: String, password: String) -> Result<(), Error> {
     let mut conn = <Connection<AsyncStdStream>>::builder()
        .enable_minecraft_quirks(true)
        .connect(address, password)
        .await?;

    rcon_send(&mut conn, "list").await?;
    Ok(())
} 

async fn rcon_send(conn: &mut Connection<AsyncStdStream>, cmd: &str) -> Result<(), Error> {
    let resp = conn.cmd(cmd).await?;
    println!("{}", resp);
    Ok(())
}
