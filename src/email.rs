use crate::models::{AbonnementInfo, MountPoint};
use lettre::{
    message::{header, MultiPart, SinglePart},
    Message, SmtpTransport, Transport,
};
use serde_derive::{Deserialize, Serialize};
use crate::models::AbonnementInfoBlattNr;

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
    pub blatt: AbonnementInfoBlattNr,
    pub webhook: String,
    pub aktenzeichen: Option<String>,
    pub aenderungs_id: String,
}

pub fn send_email(
    to: &str,
    subject: &str,
    html: &str,
    plaintext: &str,
) -> Result<(), String> {
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::transport::smtp::authentication::Mechanism;
    use lettre::transport::smtp::PoolConfig;

    println!("send_email...");
    let smtp_config = crate::db::get_email_config()?;

    let from = format!("Digitales Grundbuch <{}>", smtp_config.email);
    println!("smtp config = {:#?}", smtp_config);

    let email = Message::builder()
        .from(
            from.parse()
                .map_err(|e| format!("Ungültige Sender-E-Mail: {e}"))?,
        )
        .to(to
            .parse()
            .map_err(|e| format!("Ungültige Empfänger-E-Mail: {e}"))?)
        .subject(subject)
        .multipart(
            MultiPart::alternative() // This is composed of two parts.
                .singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::TEXT_PLAIN)
                        .body(plaintext.to_string()),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::TEXT_HTML)
                        .body(html.to_string()),
                ),
        )
        .map_err(|_| format!("Ungültige E-Mail"))?;

    println!("email {:#?}", email);

    let mailer = SmtpTransport::starttls_relay(&smtp_config.smtp_adresse)
        .map_err(|e| format!("{e}"))?
        .credentials(Credentials::new(
            smtp_config.email.clone(),
            smtp_config.passwort.clone(),
        ))
        .authentication(vec![Mechanism::Plain, Mechanism::Login, Mechanism::Xoauth2])
        .pool_config(PoolConfig::new().max_size(20))
        .build();

    println!("mailer");

    let r = mailer
        .send(&email)
        .map_err(|e| format!("failed to deliver message: {e}"));

    println!("{:#?}", r);
    let r = r?;

    Ok(())
}

pub fn send_zugriff_gewaehrt_email(
    to: &str,
    zugriff_id: &str,
    // (Land, Amtsgericht, Blatt, Nr.)
    grundbuecher: &[(String, String, String, String)],
) -> Result<(), String> {
    let server_url = crate::db::get_server_address(MountPoint::Local)?;

    let mut gb_short = grundbuecher
        .first()
        .map(|(_, a, g, b)| format!("{g} Blatt {b}"))
        .ok_or(format!("Kein Grundbuch für Zugriff"))?;

    if grundbuecher.len() > 1 {
        gb_short.push_str(" u.a.");
    }

    let gb_list_plain = grundbuecher
        .iter()
        .map(|(_, a, g, b)| format!("Amtsgericht {a}, Grundbuch von {g} Blatt {b}"))
        .collect::<Vec<_>>()
        .join("\r\n");

    let gb_list = grundbuecher
        .iter()
        .map(|(_, a, g, b)| format!("<li>Amtsgericht {a}, Grundbuch von {g} Blatt {b}</li>"))
        .collect::<Vec<_>>()
        .join("\r\n");

    let html = format!("<!DOCTYPE html>
    <html lang=\"de\">
    <head>
        <meta charset=\"UTF-8\">
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">
        <title>Ihr Zugriff auf Grundbuch {gb_short} wurde gewährt</title>
    </head>
    <body>
        <div style=\"width: 800px; margin: 0 auto;\">
          
            <h4>Ihr Zugriff auf Grundbuch {gb_short} wurde gewährt</h4>
            
            <p>Guten Tag,</p>
            
            <p>Ihr Zugriff auf die folgenden Grundbücher wurde gewährt</p>
            
            <ul>
                {gb_list}
            </ul>
            
            <p>Um ihre Grundbücher einzusehen, melden Sie sich bitte in Ihrem Konto mit dem folgenden Link an:</p>
            <a href=\"{server_url}/konto?id={zugriff_id}\">{server_url}/konto?id={zugriff_id}</a>
            <br/>
            
            <br/>

            <p>Sie wurden benachrichtigt, da Sie Benachrichtigungen für Zugriffe abonniert haben.</p>
            <p>Die Einstellung für Benachrichtigungen können Sie in Ihrem Konto über \"Einstellungen\" anpassen.</p>
        </div>
    </body>
    </html>");

    let plaintext = format!(
        "Guten Tag,

Ihr Zugriff auf die folgenden Grundbücher wurde gewährt

{gb_list_plain}

Um ihre Grundbücher einzusehen, melden Sie sich bitte in Ihrem Konto mit dem folgenden Link an:

Um die Grundbuchänderung in Code-Form einzusehen, folgen Sie bitten dem folgenden Link:
{server_url}/konto?id={zugriff_id}

Sie wurden benachrichtigt, da Sie Benachrichtigungen für Zugriffe abonniert haben.
Die Einstellung für Benachrichtigungen können Sie in Ihrem Konto über \"Einstellungen\" anpassen."
    );

    let url = reqwest::Url::parse(&server_url).map_err(|e| format!("{e}"))?;

    let host = url.host_str().unwrap_or("");

    // Die meisten SMTP-Server überprüfen die Sender-Adresse,
    // E-Mail muss mit den Login-Daten übereinstimmen
    let smtp_config = crate::db::get_email_config()?;

    send_email(
        to,
        &format!("Ihr Zugriff auf Grundbuch {gb_short} wurde gewährt"),
        &html,
        &plaintext,
    )?;

    Ok(())
}

pub fn send_zugriff_abgelehnt_email(to: &str) -> Result<(), String> {
    let server_url = crate::db::get_server_address(MountPoint::Local)?;

    Ok(())
}

pub fn send_change_email(abo: &AbonnementInfo, commit_id: &str) -> Result<(), String> {
    let AbonnementInfo {
        id,
        amtsgericht,
        blatt,
        text,
        grundbuchbezirk,
        aktenzeichen,
    } = abo;

    let blatt = match blatt {
        AbonnementInfoBlattNr::Alle => "*".to_string(),
        AbonnementInfoBlattNr::Exakt(i) => i.to_string(),
    };
    let aktenzeichen = aktenzeichen.clone().unwrap_or_default(); // TODO
    let email = text;
    let email_url = urlencoding::encode(text);
    let server_url = crate::db::get_server_address(MountPoint::Local)?;

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
            
            <p>Um die volle Grundbuchänderung in PDF-Form einzusehen, folgen Sie bitten dem folgenden Link:</p>
            <a href=\"{server_url}/aenderung/pdf/{commit_id}\">{server_url}/aenderung/pdf/{commit_id}</a>
            <br/>
            
            <p>Sie wurden benachrichtigt, da Sie diese Grundbuchblätter abonniert haben.</p>
            <p>Um das Abonnement zu kündigen, klicken Sie bitte <a href=\"{server_url}/abo-loeschen/{id}\">hier</a>.</p>
        </div>
    </body>
    </html>");

    let plaintext = format!("Guten Tag,

in den folgenden Grundbuchblättern sind Änderungen vorgenommen worden:

Amtsgericht {amtsgericht}, Bezirk {grundbuchbezirk}, Blatt {blatt}
Ihr Zeichen: {aktenzeichen}

Um die volle Grundbuchänderung in PDF-Form einzusehen, folgen Sie bitten dem folgenden Link:
{server_url}/aenderung/pdf/{commit_id}?email={email_url}

Um die Grundbuchänderung in Code-Form einzusehen, folgen Sie bitten dem folgenden Link:
{server_url}/aenderung/diff/{commit_id}?email={email_url}

Sie wurden benachrichtigt, da Sie diese Grundbuchblätter abonniert haben.
Um das Abonnement zu kündigen, klicken Sie bitte hier:
{server_url}/abo-loeschen/{id}
    ");

    let to = email;

    send_email(
        to,
        &format!(
            "Grundbuchänderung in {grundbuchbezirk} Blatt {blatt} (Aktenzeichen {aktenzeichen})"
        ),
        &html,
        &plaintext,
    )?;

    Ok(())
}

pub async fn send_change_webhook(abo: &AbonnementInfo, commit_id: &str) -> Result<(), String> {
    let server_url = crate::db::get_server_address(MountPoint::Local)?;

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
    let _ = client
        .post(&abo.text)
        .json(&abo_info)
        .send()
        .await
        .map_err(|e| format!("{e}"))?;
    Ok(())
}
