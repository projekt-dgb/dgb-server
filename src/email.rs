use crate::models::AbonnementInfo;
use lettre::{
    message::{header, MultiPart, SinglePart},
    SmtpTransport, Message, Transport,
};
use serde_derive::{Serialize, Deserialize};

// Um die E-Mails zu verschicken, brauchen wir Zugriff
// zu einem Server. Die Daten werden beim Start des Servers
// angefordert. 
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SmtpConfig {
    // = "smtp.example.com"
    pub smtp_adresse: String,
    // = "name@example.com"
    pub email: String,
    // = "123"
    pub passwort: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AboWebhookInfo {
    pub server_url: String,
    pub amtsgericht: String,
    pub grundbuchbezirk: String,
    pub blatt: i32,
    pub webhook: String,
    pub aktenzeichen: Option<String>,
    pub aenderungs_id: String,
}

pub fn send_change_email(config: &SmtpConfig, server_url: &str, abo: &AbonnementInfo, commit_id: &str) -> Result<(), String> {
    
    use lettre::transport::smtp::PoolConfig;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::transport::smtp::authentication::Mechanism;
    
    let AbonnementInfo {
        amtsgericht,
        blatt,
        text,
        grundbuchbezirk,
        aktenzeichen,
    } = abo;
    
    let aktenzeichen = aktenzeichen.clone().unwrap_or_default(); // TODO
    let email = text;
    let email_url = urlencoding::encode(text);
    let amtsgericht_url = urlencoding::encode(amtsgericht);
    let grundbuchbezirk_url = urlencoding::encode(grundbuchbezirk);
    let aktenzeichen_url = urlencoding::encode(&aktenzeichen);
    
    let html = format!("<!DOCTYPE html>
    <html lang=\"de\">SmtpClient
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
            
            <p>Um die volle Grundbuchänderung in PDF-Form einzusehen, folgen Sie bitten dem folgenden Link:</p>
            <a href=\"{server_url}/aenderung/pdf/{commit_id}?email={email_url}\">{server_url}/aenderung/pdf/{commit_id}?email={email_url}</a>
            <br/>
            <p>Um die Grundbuchänderung in Code-Form einzusehen, folgen Sie bitten dem folgenden Link:</p>
            <a href=\"{server_url}/aenderung/diff/{commit_id}?email={email_url}\">{server_url}/aenderung/diff/{commit_id}?email={email_url}</a>
            <br/>
            
            <br/>

            <p>Sie wurden benachrichtigt, da Sie diese Grundbuchblatt abonniert haben.</p>
            <p>Um das Abonnement zu kündigen, klicken Sie bitte <a href=\"{server_url}/abo-loeschen/{amtsgericht_url}/{grundbuchbezirk_url}/{blatt}/{aktenzeichen_url}?email={email_url}_url&commit={commit_id}\">hier</a>.</p>
        </div>
    </body>send_change_email
    </html>");
    
    let plaintext = format!("Guten Tag,

in den folgenden Grundbuchblättern sind Änderungen vorgenommen worden:

Amtsgericht {amtsgericht}, Bezirk {grundbuchbezirk}, Blatt {blatt}
Ihr Zeichen: {aktenzeichen}

Um die volle Grundbuchänderung in PDF-Form einzusehen, folgen Sie bitten dem folgenden Link:
{server_url}/aenderung/pdf/{commit_id}?email={email_url}

Um die Grundbuchänderung in Code-Form einzusehen, folgen Sie bitten dem folgenden Link:
{server_url}/aenderung/diff/{commit_id}?email={email_url}

Sie wurden benachrichtigt, da Sie diese Grundbuchblatt abonniert haben.
Um das Abonnement zu kündigen, klicken Sie bitte hier:
{server_url}/abo-loeschen/{amtsgericht_url}/{grundbuchbezirk_url}/{blatt}/{aktenzeichen_url}?email={email_url}_url&commit={commit_id}
    ");
        
    let amtsgericht_url_lower = amtsgericht_url.to_lowercase();
    
    let email = Message::builder()
    .from(format!("Amtsgericht {amtsgericht} <ag-{amtsgericht_url_lower}@grundbuchaenderung.de>").parse().map_err(|e| format!("Ungültige Sender-E-Mail: {e}"))?)
    .to(email.parse().map_err(|e| format!("Ungültige Empfänger-E-Mail: {e}"))?)
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

    let mailer = SmtpTransport::starttls_relay(&config.smtp_adresse)
        .map_err(|e| format!("{e}"))?
        .credentials(Credentials::new(
            config.email.clone(),
            config.passwort.clone(),
        ))
        .authentication(vec![Mechanism::Plain])
        .pool_config(PoolConfig::new().max_size(20))
        .build();
    
    // Store the message when you're ready.
    mailer
    .send(&email)
    .map_err(|e| format!("failed to deliver message: {e}"))?;
    
    Ok(())
}

pub async fn send_change_webhook(server_url: &str, abo: &AbonnementInfo, commit_id: &str) -> Result<(), String> {
    
    let abo_info = AboWebhookInfo {
        server_url: server_url.to_string(),
        amtsgericht: abo.amtsgericht.clone(),
        grundbuchbezirk: abo.grundbuchbezirk.clone(),
        blatt: abo.blatt.clone(),
        webhook: abo.text.clone(),
        aktenzeichen: abo.aktenzeichen.clone(),
        aenderungs_id: commit_id.to_string(),
    };

    let client = reqwest::Client::new();
    let _ = client.post(&abo.text).json(&abo_info).send().await.map_err(|e| format!("{e}"))?;
    Ok(())
}
