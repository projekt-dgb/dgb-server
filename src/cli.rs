use crate::api::commit::DbChangeOp;
use crate::db::GpgPublicKeyPair;
use crate::{
    AboLoeschenArgs, AboNeuArgs, BenutzerLoeschenArgs, BenutzerNeuArgsCli, BezirkLoeschenArgs,
    BezirkNeuArgs, SchluesselNeuArgs,
};
use std::path::Path;

pub async fn create_bezirk_cli(args: &BezirkNeuArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BezirkNeu(args.clone()), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn delete_bezirk_cli(args: &BezirkLoeschenArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BezirkLoeschen(args.clone()), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub fn schluessel_neu(args: &SchluesselNeuArgs) -> Result<(), anyhow::Error> {
    let gpg_key_pair =
        crate::db::create_gpg_key(&args.name, &args.email).map_err(|e| anyhow::anyhow!("{e}"))?;
    let out_dir = args.dir.clone().unwrap_or(
        Path::new("./keys")
            .to_path_buf()
            .canonicalize()
            .unwrap_or_default(),
    );
    let _ = std::fs::create_dir_all(&out_dir);

    let private_key_out_file = out_dir.join(&format!("{}.private.gpg", args.email));
    std::fs::write(
        private_key_out_file.clone(),
        gpg_key_pair.private.clone().join("\r\n"),
    )?;
    println!("Privater Schlüssel => {private_key_out_file:?}");

    let public_out_file = serde_json::to_string_pretty(&GpgPublicKeyPair {
        fingerprint: gpg_key_pair.fingerprint.clone(),
        public: gpg_key_pair.public.clone(),
    })
    .unwrap_or_default();
    let public_key_out_file = out_dir.join(&format!("{}.public.gpg.json", args.email));
    std::fs::write(public_key_out_file.clone(), public_out_file)?;
    println!("Öffentlicher Schlüssel => {public_key_out_file:?}");

    Ok(())
}

pub async fn create_user_cli(args: &BenutzerNeuArgsCli) -> Result<(), anyhow::Error> {
    let benutzer_args_json = args.into_json()?;
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BenutzerNeu(benutzer_args_json), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn delete_user_cli(args: &BenutzerLoeschenArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BenutzerLoeschen(args.clone()), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn create_abo_cli(args: &AboNeuArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::AboNeu(args.clone()), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn delete_abo_cli(args: &AboLoeschenArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::AboLoeschen(args.clone()), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}
