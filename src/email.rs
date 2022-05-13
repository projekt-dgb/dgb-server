use crate::models::AbonnementInfo;
use lettre::{
    message::{header, MultiPart, SinglePart},
    FileTransport, Message, Transport,
};
use serde_derive::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AboWebhookInfo {
    pub server_url: String,
    pub amtsgericht: String,
    pub grundbuchbezirk: String,
    pub blatt: i32,
    pub webhook: String,
    pub aktenzeichen: String,
    pub commit_id: String,
}

pub fn send_change_email(server_url: &str, abo: &AbonnementInfo) -> Result<(), String> {
    
    let AbonnementInfo {
        amtsgericht,
        blatt,
        text,
        grundbuchbezirk,
        aktenzeichen,
        commit_id,
    } = abo;
    
    let email = text;
    let email_url = urlencoding::encode(text);
    let amtsgericht_url = urlencoding::encode(amtsgericht);
    let grundbuchbezirk_url = urlencoding::encode(grundbuchbezirk);
    let aktenzeichen_url = urlencoding::encode(aktenzeichen);
    
    let html = format!("<!DOCTYPE html>
    <html lang=\"de\">
    <head>
        <meta charset=\"UTF-8\">
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">
        <title>Grundbuchänderung in {grundbuchbezirk} Blatt {blatt} (Aktenzeichen {aktenzeichen})</title>
    </head>
    <body>
        <div style=\"width: 800px; margin: 0 auto;\">
          
            <h4>Grundbuchänderung in {grundbuchbezirk} Blatt {blatt} (Aktenzeichen {aktenzeichen})</h4>
            
            <p>Guten Tag,</p>
            
            <p>in den folgenden Grundbuchblättern sind Änderungen vorgenommen worden:</p>
            
            <ul>
                <li>Amtsgericht {amtsgericht}, Bezirk {grundbuchbezirk}, Blatt {blatt}</li>
                <li>Ihr Zeichen: {aktenzeichen}</li>
            </ul>
            
            <p>Um die volle Grundbuchänderung in PDF-Form einzusehen, folgen Sie bitten dem folgenden Link:</p><br/>
            <a href=\"{server_url}/aenderung/pdf/{commit_id}?email={email_url}\">{server_url}/aenderung/pdf/{commit_id}?email={email_url}</a>
            <br/>
            <p>Um die Grundbuchänderung in Code-Form einzusehen, folgen Sie bitten dem folgenden Link:</p><br/>
            <a href=\"{server_url}/aenderung/code/{commit_id}?email={email_url}\">{server_url}/aenderung/pdf/{commit_id}?email={email_url}</a>
            <br/>
            
            <br/>

            <p>Sie wurden benachrichtigt, da Sie diese Grundbuchblatt abonniert haben.</p>
            <p>Um das Abonnement zu kündigen, klicken Sie bitte 
                <a href=\"{server_url}/abo-loeschen/{amtsgericht_url}/{grundbuchbezirk_url}/{blatt}/{aktenzeichen_url}?email={email_url}_url&commit={commit_id}\">hier</a>
            .</p>
        </div>
    </body>
    </html>");
    
    let plaintext = format!("**Grundbuchänderung in {grundbuchbezirk} Blatt {blatt} (Aktenzeichen {aktenzeichen})**
            
Guten Tag,

in den folgenden Grundbuchblättern sind Änderungen vorgenommen worden:

Amtsgericht {amtsgericht}, Bezirk {grundbuchbezirk}, Blatt {blatt}
Ihr Zeichen: {aktenzeichen}

Um die volle Grundbuchänderung in PDF-Form einzusehen, folgen Sie bitten dem folgenden Link:
{server_url}/aenderung/pdf/{commit_id}?email={email_url}

Um die Grundbuchänderung in Code-Form einzusehen, folgen Sie bitten dem folgenden Link:
{server_url}/aenderung/pdf/{commit_id}?email={email_url}

Sie wurden benachrichtigt, da Sie diese Grundbuchblatt abonniert haben.
Um das Abonnement zu kündigen, klicken Sie bitte hier:
{server_url}/abo-loeschen/{amtsgericht_url}/{grundbuchbezirk_url}/{blatt}/{aktenzeichen_url}?email={email_url}_url&commit={commit_id}
    ");
    
    let email = Message::builder()
    .from("Amtsgericht {amtsgericht} <ag-{amtsgericht_url}@grundbuchaenderung.de>".parse().unwrap())
    .to(email.parse().unwrap())
    .subject(&format!("Grundbuchänderung in {grundbuchbezirk} Blatt {blatt} (Aktenzeichen {aktenzeichen})"))
    .multipart(
        MultiPart::alternative() // This is composed of two parts.
            .singlepart(
                SinglePart::builder()
                    .header(header::ContentType::TEXT_PLAIN)
                    .body(plaintext),
            )
            .singlepart(
                SinglePart::builder()
                    .header(header::ContentType::TEXT_HTML)
                    .body(String::from(html)),
            ),
    )
    .map_err(|e| format!("failed to build email"))?;

    // Create our mailer. Please see the other examples for creating SMTP mailers.
    // The path given here must exist on the filesystem.
    let _ = std::fs::create_dir_all("./email");
    let mailer = FileTransport::new("./email");

    // Store the message when you're ready.
    mailer
    .send(&email)
    .map_err(|e| format!("failed to deliver message"))?;
    
    Ok(())
}

pub async fn send_change_webhook(server_url: &str, abo: &AbonnementInfo) -> Result<(), String> {
    
    let abo_info = AboWebhookInfo {
        server_url: server_url.to_string(),
        amtsgericht: abo.amtsgericht.clone(),
        grundbuchbezirk: abo.grundbuchbezirk.clone(),
        blatt: abo.blatt.clone(),
        webhook: abo.text.clone(),
        aktenzeichen: abo.aktenzeichen.clone(),
        commit_id: abo.commit_id.clone(),
    };

    let client = reqwest::Client::new();
    let _ = client.post(&abo.text).json(&abo_info).send().await.map_err(|e| format!("{e}"))?;
    Ok(())
}
