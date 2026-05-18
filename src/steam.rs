use std::path::PathBuf;

/// Figure out the Steam folder to use based on its explicitly given path
/// or try to guess it based on the default location.
fn find_steam_folder(
    steam_folder: &Option<PathBuf>,
    home_dir: &Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    if let Some(steam_folder) = steam_folder {
        if !steam_folder.is_dir() {
            anyhow::bail!(
                "--steam-library-root {} is not a valid directory",
                steam_folder.display()
            );
        }

        return Ok(steam_folder.clone());
    }

    if let Some(home_dir) = home_dir
        && home_dir.is_dir()
        && home_dir.is_absolute()
    {
        let default_locations = [
            // default library folder on regular Linux desktop
            home_dir.join(".local/share/Steam"),
            // supposedly the default library folder on Steam Deck
            home_dir.join(".steam/steam"),
        ];

        if let Some(steam_folder) = default_locations.into_iter().find(|p| p.is_dir()) {
            return Ok(steam_folder);
        }
    }

    anyhow::bail!(
        "Could not find EaW installation directory. Please specify --eaw-root or --steam-library-root."
    )
}

/// Figure out the EaW game folder based on its explicitly given path
/// or the Steam folder it is installed in.
pub fn find_eaw_root(
    eaw_root: Option<PathBuf>,
    steam_folder: &Option<PathBuf>,
    home_dir: &Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    if let Some(eaw_root) = eaw_root {
        if !eaw_root.is_dir() {
            anyhow::bail!("--eaw-root {} is not a valid directory", eaw_root.display());
        }

        return Ok(eaw_root);
    }

    let steam_folder = find_steam_folder(steam_folder, home_dir)?;

    let eaw_steam_root = steam_folder.join::<PathBuf>(
        ["steamapps", "common", "Star Wars Empire at War"]
            .iter()
            .collect(),
    );

    if eaw_steam_root.is_dir() {
        return Ok(eaw_steam_root);
    }

    anyhow::bail!(
        "{} is not a valid EaW installation directory. Please specify --eaw-root or --steam-library-root.",
        eaw_steam_root.display()
    )
}

/// Figure out the source folder of the mod to repack based on its explicitly given path
/// or Steam workshop ID.
pub fn find_source_mod_folder(
    steam_mod_id: Option<String>,
    steam_folder: &Option<PathBuf>,
    home_dir: &Option<PathBuf>,
    source_dir: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    if let Some(steam_mod_id) = steam_mod_id {
        if source_dir.is_some() {
            anyhow::bail!("Only one of --steam-mod-id or --source-dir may be specified.");
        }

        let steam_folder = find_steam_folder(steam_folder, home_dir)?;
        let steam_mod_folder = steam_folder.join::<PathBuf>(
            [
                "steamapps",
                "workshop",
                "content",
                "32470",
                steam_mod_id.as_str(),
            ]
            .iter()
            .collect(),
        );
        if steam_mod_folder.is_dir() {
            return Ok(steam_mod_folder);
        }

        anyhow::bail!(
            "{} is not a valid Steam mod folder. Check your --steam-library-root and --steam-mod-id options.",
            steam_mod_folder.display()
        );
    }

    if let Some(source_dir) = source_dir {
        if source_dir.is_dir() {
            return Ok(source_dir);
        }

        anyhow::bail!("{} is not a valid mod folder.", source_dir.display());
    }

    anyhow::bail!("Exactly one of --steam-mod-id or --source-dir is required.")
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs};

    use tempfile::TempDir;

    use crate::steam::{find_eaw_root, find_source_mod_folder};

    #[test]
    fn fails_if_cannot_find_eaw_root() {
        let err = find_eaw_root(None, &None, &None).expect_err("should be an error");

        assert_eq!(
            err.to_string(),
            "Could not find EaW installation directory. Please specify --eaw-root or --steam-library-root."
        );
    }

    #[test]
    fn fails_if_eaw_root_arguments_are_invalid() {
        let tmp_dir = TempDir::new().expect("failed to create test dir");
        let bad_path = tmp_dir.path().join("bad");
        let bad_eaw_root =
            find_eaw_root(Some(bad_path.clone()), &None, &None).expect_err("should be an error");
        let bad_steam_folder =
            find_eaw_root(None, &Some(bad_path.clone()), &None).expect_err("should be an error");

        assert_eq!(
            bad_eaw_root.to_string(),
            format!("--eaw-root {} is not a valid directory", bad_path.display(),)
        );
        assert_eq!(
            bad_steam_folder.to_string(),
            format!(
                "--steam-library-root {} is not a valid directory",
                bad_path.display(),
            )
        );
    }

    #[test]
    fn uses_explicit_eaw_root() -> Result<(), Box<dyn Error>> {
        let tmp_dir = TempDir::new().expect("failed to create test dir");
        let expected_eaw_root = tmp_dir.path().join("SWEAW");
        let steam_folder = tmp_dir.path().join("Steam");
        fs::create_dir(&expected_eaw_root)?;
        fs::create_dir(&steam_folder)?;

        let eaw_root = find_eaw_root(Some(expected_eaw_root.clone()), &None, &None)?;
        let eaw_root_with_steam_folder =
            find_eaw_root(Some(expected_eaw_root.clone()), &Some(steam_folder), &None)?;

        assert_eq!(expected_eaw_root, eaw_root, "should use given EaW root dir");
        assert_eq!(
            expected_eaw_root, eaw_root_with_steam_folder,
            "should use given EaW root dir even if Steam folder was given"
        );

        Ok(())
    }

    #[test]
    fn infers_eaw_root_via_steam_folder() -> Result<(), Box<dyn Error>> {
        let tmp_dir = TempDir::new().expect("failed to create test dir");
        let home_dir = tmp_dir.path().join("home");
        let steam_folder = tmp_dir.path().join("Steam");

        let home_eaw_root =
            home_dir.join(".local/share/Steam/steamapps/common/Star Wars Empire at War");

        let steam_eaw_root = steam_folder.join("steamapps/common/Star Wars Empire at War");

        fs::create_dir_all(&home_eaw_root)?;
        fs::create_dir_all(&steam_eaw_root)?;

        let eaw_root_via_homedir = find_eaw_root(None, &None, &Some(home_dir))?;
        let eaw_root_via_steam_folder = find_eaw_root(None, &Some(steam_folder), &None)?;

        assert_eq!(
            home_eaw_root, eaw_root_via_homedir,
            "should infer EaW root dir via home -> Steam"
        );
        assert_eq!(
            steam_eaw_root, eaw_root_via_steam_folder,
            "should infer EaW root dir via Steam folder"
        );

        Ok(())
    }

    #[test]
    fn infers_eaw_root_on_steam_deck() -> Result<(), Box<dyn Error>> {
        let tmp_dir = TempDir::new().expect("failed to create test dir");
        let home_dir = tmp_dir.path().join("home");

        let home_eaw_root = home_dir.join(".steam/steam/steamapps/common/Star Wars Empire at War");

        fs::create_dir_all(&home_eaw_root)?;

        let eaw_root_via_homedir = find_eaw_root(None, &None, &Some(home_dir))?;

        assert_eq!(
            home_eaw_root, eaw_root_via_homedir,
            "should infer EaW root dir via home -> Steam on Steam Deck"
        );

        Ok(())
    }

    #[test]
    fn find_source_mod_folder_complains_if_given_incorrect_option_combo() {
        let both = find_source_mod_folder(Some("1234".into()), &None, &None, Some("bar".into()))
            .expect_err("should be an error");
        let neither =
            find_source_mod_folder(None, &None, &None, None).expect_err("should be an error");

        assert_eq!(
            both.to_string(),
            "Only one of --steam-mod-id or --source-dir may be specified."
        );
        assert_eq!(
            neither.to_string(),
            "Exactly one of --steam-mod-id or --source-dir is required."
        );
    }

    #[test]
    fn find_source_mod_folder_fails_if_arguments_are_invalid() -> Result<(), Box<dyn Error>> {
        let tmp_dir = TempDir::new().expect("failed to create test dir");
        let steam = tmp_dir.path().join("Steam");
        let bad_path = tmp_dir.path().join("bad");

        fs::create_dir_all(&steam)?;

        let bad_source = find_source_mod_folder(None, &None, &None, Some(bad_path.clone()))
            .expect_err("should be an error");
        let bad_steam_folder =
            find_source_mod_folder(Some("2123".into()), &Some(bad_path.clone()), &None, None)
                .expect_err("should be an error");
        let bad_steam_mod =
            find_source_mod_folder(Some("2123".into()), &Some(steam.clone()), &None, None)
                .expect_err("should be an error");

        assert_eq!(
            bad_source.to_string(),
            format!("{} is not a valid mod folder.", bad_path.display())
        );
        assert_eq!(
            bad_steam_folder.to_string(),
            format!(
                "--steam-library-root {} is not a valid directory",
                bad_path.display(),
            )
        );
        assert_eq!(
            bad_steam_mod.to_string(),
            format!(
                "{} is not a valid Steam mod folder. Check your --steam-library-root and --steam-mod-id options.",
                steam
                    .join("steamapps/workshop/content/32470/2123")
                    .display(),
            )
        );

        Ok(())
    }

    #[test]
    fn find_source_mod_folder_uses_source_or_steam_folder() -> Result<(), Box<dyn Error>> {
        let tmp_dir = TempDir::new().expect("failed to create test dir");
        let steam_mod_path = tmp_dir
            .path()
            .join("Steam/steamapps/workshop/content/32470/2123");
        let source_path = tmp_dir.path().join("TestMod");

        fs::create_dir_all(&steam_mod_path)?;
        fs::create_dir_all(&source_path)?;

        let steam_mod_folder = find_source_mod_folder(
            Some("2123".into()),
            &Some(tmp_dir.path().join("Steam")),
            &None,
            None,
        )?;
        let source_mod_folder =
            find_source_mod_folder(None, &None, &None, Some(source_path.clone()))?;

        assert_eq!(
            steam_mod_folder, steam_mod_path,
            "should derive Steam mod folder using workshop ID and Steam folder"
        );
        assert_eq!(
            source_mod_folder, source_path,
            "should use given source dir if valid"
        );

        Ok(())
    }
}
