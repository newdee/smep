//! Make smep the program that opens Markdown files, as far as each system
//! lets a program do that for itself.
//!
//! - macOS lets an application claim a document type outright
//!   (LaunchServices), but only an application bundle has the identity to
//!   claim with: the `smep.app` from a release, not a bare binary.
//! - Windows lets a program register itself and its file types, and keeps
//!   the choice of a default for the user: smep registers, then opens the
//!   Settings page where smep is one click away.
//! - Linux desktops read `.desktop` files; smep writes its own and asks
//!   `xdg-mime` to make it the handler for Markdown.

use anyhow::Result;

/// What happened, for the dialog afterwards.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Whether smep is now the default, or a step is left to the user.
    pub done: bool,
    pub message: String,
}

/// Try to become the default for Markdown files on this system.
pub fn make_default() -> Result<Outcome> {
    let exe = std::env::current_exe()?;
    platform::make_default(&exe)
}

#[cfg(windows)]
mod platform {
    use std::io;
    use std::path::Path;
    use std::process::Command;

    use anyhow::{Context as _, Result};
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    use super::Outcome;

    pub const PROG_ID: &str = "smep.markdown";
    pub const APP: &str = "smep";
    pub const EXTENSIONS: [&str; 4] = [".md", ".markdown", ".html", ".htm"];

    /// Register under `root` (`HKEY_CURRENT_USER` in use, a scratch key in
    /// tests): the ProgId with its open command, the extensions that offer
    /// it under "Open with", and the application with its capabilities, so
    /// Settings > Default apps lists smep by name.
    pub fn register(root: &RegKey, exe: &Path) -> io::Result<()> {
        let exe = exe.display();
        let classes = root.create_subkey(r"Software\Classes")?.0;

        let prog = classes.create_subkey(PROG_ID)?.0;
        prog.set_value("", &"Markdown document (smep)")?;
        prog.set_value("FriendlyTypeName", &"Markdown document")?;
        prog.create_subkey("DefaultIcon")?
            .0
            .set_value("", &format!("\"{exe}\",0"))?;
        prog.create_subkey(r"shell\open\command")?
            .0
            .set_value("", &format!("\"{exe}\" \"%1\""))?;

        for ext in EXTENSIONS {
            classes
                .create_subkey(format!(r"{ext}\OpenWithProgids"))?
                .0
                .set_value(PROG_ID, &"")?;
        }

        let capabilities = root
            .create_subkey(format!(r"Software\{APP}\Capabilities"))?
            .0;
        capabilities.set_value("ApplicationName", &"smep")?;
        capabilities.set_value(
            "ApplicationDescription",
            &"A Simple Markdown Editor & Previewer",
        )?;
        let associations = capabilities.create_subkey("FileAssociations")?.0;
        for ext in EXTENSIONS {
            associations.set_value(ext, &PROG_ID)?;
        }
        root.create_subkey(r"Software\RegisteredApplications")?
            .0
            .set_value(APP, &format!(r"Software\{APP}\Capabilities"))?;
        Ok(())
    }

    /// Undo [`register`]; missing keys are fine. (The app has no menu item
    /// for this yet; `scripts/register-windows.ps1 -Remove` does the same.)
    #[cfg(test)]
    pub fn unregister(root: &RegKey) -> io::Result<()> {
        if let Ok(classes) =
            root.open_subkey_with_flags(r"Software\Classes", winreg::enums::KEY_ALL_ACCESS)
        {
            let _ = classes.delete_subkey_all(PROG_ID);
            for ext in EXTENSIONS {
                if let Ok(open_with) = classes.open_subkey_with_flags(
                    format!(r"{ext}\OpenWithProgids"),
                    winreg::enums::KEY_ALL_ACCESS,
                ) {
                    let _ = open_with.delete_value(PROG_ID);
                }
            }
        }
        if let Ok(software) = root.open_subkey_with_flags("Software", winreg::enums::KEY_ALL_ACCESS)
        {
            let _ = software.delete_subkey_all(APP);
            if let Ok(registered) = software
                .open_subkey_with_flags("RegisteredApplications", winreg::enums::KEY_ALL_ACCESS)
            {
                let _ = registered.delete_value(APP);
            }
        }
        Ok(())
    }

    pub fn make_default(exe: &Path) -> Result<Outcome> {
        register(&RegKey::predef(HKEY_CURRENT_USER), exe)
            .context("could not write smep's registration to the registry")?;
        // Windows keeps the default itself; this opens the page for smep.
        let settings = format!("ms-settings:defaultapps?registeredAppUser={APP}");
        let opened = Command::new("cmd")
            .args(["/C", "start", "", &settings])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        Ok(Outcome {
            done: false,
            message: if opened {
                "smep is registered for .md, .markdown, .html and .htm. In the Settings page that opened, choose smep next to .md (and the others you want).".to_string()
            } else {
                "smep is registered for .md, .markdown, .html and .htm. Now open Settings > Apps > Default apps > smep and choose it next to .md.".to_string()
            },
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn registers_the_progid_the_extensions_and_the_application() {
            let scratch = format!(r"Software\smep-test-{}", std::process::id());
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let root = hkcu.create_subkey(&scratch).unwrap().0;

            register(&root, Path::new(r"C:\tools\smep.exe")).unwrap();
            let command: String = root
                .open_subkey(format!(r"Software\Classes\{PROG_ID}\shell\open\command"))
                .unwrap()
                .get_value("")
                .unwrap();
            assert_eq!(command, r#""C:\tools\smep.exe" "%1""#);
            let open_with: String = root
                .open_subkey(r"Software\Classes\.md\OpenWithProgids")
                .unwrap()
                .get_value(PROG_ID)
                .unwrap();
            assert_eq!(open_with, "");
            let association: String = root
                .open_subkey(r"Software\smep\Capabilities\FileAssociations")
                .unwrap()
                .get_value(".md")
                .unwrap();
            assert_eq!(association, PROG_ID);
            let registered: String = root
                .open_subkey(r"Software\RegisteredApplications")
                .unwrap()
                .get_value(APP)
                .unwrap();
            assert_eq!(registered, r"Software\smep\Capabilities");

            // Twice is fine, and undoing leaves nothing of smep behind.
            register(&root, Path::new(r"C:\tools\smep.exe")).unwrap();
            unregister(&root).unwrap();
            assert!(
                root.open_subkey(format!(r"Software\Classes\{PROG_ID}"))
                    .is_err()
            );
            assert!(root.open_subkey(r"Software\smep").is_err());
            let registered = root
                .open_subkey(r"Software\RegisteredApplications")
                .unwrap();
            assert!(registered.get_value::<String, _>(APP).is_err());

            hkcu.delete_subkey_all(&scratch).unwrap();
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::Path;

    use anyhow::{Result, bail};
    use core_foundation::base::TCFType as _;
    use core_foundation::bundle::CFBundle;
    use core_foundation::string::{CFString, CFStringRef};

    use super::Outcome;

    /// The uniform type of `.md` files.
    const MARKDOWN: &str = "net.daringfireball.markdown";
    const ROLES_ALL: u32 = 0xFFFF_FFFF;

    #[link(name = "CoreServices", kind = "framework")]
    unsafe extern "C" {
        fn LSSetDefaultRoleHandlerForContentType(
            content_type: CFStringRef,
            role: u32,
            handler_bundle_id: CFStringRef,
        ) -> i32;
    }

    /// The running application's bundle identifier, if it runs from a
    /// bundle at all.
    fn bundle_identifier() -> Option<String> {
        let bundle = CFBundle::main_bundle();
        let info = bundle.info_dictionary();
        let key = CFString::from_static_string("CFBundleIdentifier");
        let value = info.find(key.as_CFType())?;
        let value = value.downcast::<CFString>()?;
        Some(value.to_string())
    }

    pub fn make_default(exe: &Path) -> Result<Outcome> {
        let Some(bundle_id) = bundle_identifier() else {
            bail!(
                "this smep ({}) runs outside an application bundle, and only an application can claim a file type; use the smep.app from a release (https://github.com/newdee/smep/releases)",
                exe.display()
            );
        };
        let status = unsafe {
            LSSetDefaultRoleHandlerForContentType(
                CFString::new(MARKDOWN).as_concrete_TypeRef(),
                ROLES_ALL,
                CFString::new(&bundle_id).as_concrete_TypeRef(),
            )
        };
        if status != 0 {
            bail!("LaunchServices refused (status {status})");
        }
        Ok(Outcome {
            done: true,
            message: "smep now opens Markdown files from Finder.".to_string(),
        })
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use anyhow::{Context as _, Result, bail};

    use super::Outcome;

    pub const FILE_NAME: &str = "smep.desktop";
    const MIME_TYPES: [&str; 2] = ["text/markdown", "text/x-markdown"];

    /// The desktop entry, pointing at `exe`.
    pub fn desktop_entry(exe: &Path) -> String {
        format!(
            "[Desktop Entry]\nType=Application\nName=smep\nGenericName=Markdown Editor\nComment=A Simple Markdown Editor & Previewer, written in Rust\nExec={} %f\nIcon=text-markdown\nTerminal=false\nCategories=Office;TextEditor;Utility;\nMimeType=text/markdown;text/x-markdown;text/html;\nKeywords=markdown;editor;preview;\n",
            exe.display()
        )
    }

    /// Write the entry into `applications` (`~/.local/share/applications`
    /// in use). Returns the file's path.
    pub fn install(applications: &Path, exe: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(applications)?;
        let path = applications.join(FILE_NAME);
        std::fs::write(&path, desktop_entry(exe))?;
        Ok(path)
    }

    fn applications_dir() -> Option<PathBuf> {
        Some(dirs::data_dir()?.join("applications"))
    }

    pub fn make_default(exe: &Path) -> Result<Outcome> {
        let Some(applications) = applications_dir() else {
            bail!("no XDG data directory for the desktop entry");
        };
        let path = install(&applications, exe)
            .with_context(|| format!("could not write {}", applications.display()))?;
        // Not every desktop keeps the database; a failure here is harmless.
        let _ = Command::new("update-desktop-database")
            .arg(&applications)
            .status();
        let status = Command::new("xdg-mime")
            .arg("default")
            .arg(FILE_NAME)
            .args(MIME_TYPES)
            .status()
            .context("xdg-mime is not installed (package xdg-utils)")?;
        if !status.success() {
            bail!("xdg-mime default failed with {status}");
        }
        Ok(Outcome {
            done: true,
            message: format!(
                "smep now opens Markdown files ({} written, xdg-mime set).",
                path.display()
            ),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_desktop_entry_names_the_running_binary() {
            let dir = std::env::temp_dir().join(format!("smep-desktop-{}", std::process::id()));
            let path = install(&dir, Path::new("/opt/smep/bin/smep")).unwrap();
            let entry = std::fs::read_to_string(&path).unwrap();
            assert!(entry.contains("Exec=/opt/smep/bin/smep %f\n"));
            assert!(entry.contains("MimeType=text/markdown;"));
            assert_eq!(path.file_name().unwrap(), FILE_NAME);
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod platform {
    use std::path::Path;

    use anyhow::{Result, bail};

    use super::Outcome;

    pub fn make_default(_exe: &Path) -> Result<Outcome> {
        bail!("smep does not know how to register itself on this system");
    }
}
