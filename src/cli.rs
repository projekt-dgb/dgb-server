use crate::{
    BezirkNeuArgs, 
    BenutzerNeuArgsCli, 
    BezirkLoeschenArgs,
    BenutzerLoeschenArgs, 
    AboNeuArgs, 
    AboLoeschenArgs
};

pub fn create_bezirk_cli(args: &BezirkNeuArgs) -> Result<(), anyhow::Error> {
    // write_to_root_db(change: commit::DbChangeOp)
    Ok(())
}

pub fn delete_bezirk_cli(args: &BezirkLoeschenArgs) -> Result<(), anyhow::Error> {
    // write_to_root_db(change: commit::DbChangeOp)
    Ok(())
}

pub fn create_user_cli(args: &BenutzerNeuArgsCli) -> Result<(), anyhow::Error> {
    // write_to_root_db(change: commit::DbChangeOp)
    Ok(())
}

pub fn delete_user_cli(args: &BenutzerLoeschenArgs) -> Result<(), anyhow::Error> {
    Ok(())
}

pub fn create_abo_cli(args: &AboNeuArgs) -> Result<(), anyhow::Error> {
    // write_to_root_db(change: commit::DbChangeOp)
    Ok(())
}

pub fn delete_abo_cli(args: &AboLoeschenArgs) -> Result<(), anyhow::Error> {
    // write_to_root_db(change: commit::DbChangeOp)
    Ok(())
}