use crate::{
    BezirkNeuArgs, 
    BenutzerNeuArgsCli, 
    BezirkLoeschenArgs,
    BenutzerLoeschenArgs, 
    AboNeuArgs, 
    AboLoeschenArgs
};
use crate::api::commit::DbChangeOp;

pub async fn create_bezirk_cli(args: &BezirkNeuArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BezirkNeu(args.clone()), &app_state).await
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn delete_bezirk_cli(args: &BezirkLoeschenArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BezirkLoeschen(args.clone()), &app_state).await
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn create_user_cli(args: &BenutzerNeuArgsCli) -> Result<(), anyhow::Error> {
    let benutzer_args_json = args.into_json()?;
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BenutzerNeu(benutzer_args_json), &app_state).await
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn delete_user_cli(args: &BenutzerLoeschenArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BenutzerLoeschen(args.clone()), &app_state).await
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn create_abo_cli(args: &AboNeuArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::AboNeu(args.clone()), &app_state).await
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn delete_abo_cli(args: &AboLoeschenArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::AboLoeschen(args.clone()), &app_state).await
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}