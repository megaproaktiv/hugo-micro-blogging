//! Installs a macOS Quick Action ("Dienst") so that selected text anywhere on the computer can be
//! saved as a snippet via right-click → Dienste → "Als hmb-Snippet speichern".
//! The action runs this program with `--snippet` and passes the selected text on stdin.

use std::path::PathBuf;

pub const SERVICE_NAME: &str = "Als hmb-Snippet speichern";

pub fn service_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("Library/Services").join(format!("{SERVICE_NAME}.workflow")))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Names used by earlier versions (before the rename to hmb).
const OLD_SERVICE_NAMES: [&str; 1] = ["Als Microblog-Snippet speichern"];

fn old_service_dirs() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else { return vec![] };
    OLD_SERVICE_NAMES
        .iter()
        .map(|n| home.join("Library/Services").join(format!("{n}.workflow")))
        .filter(|d| d.join("Contents/document.wflow").is_file())
        .collect()
}

#[derive(Debug, PartialEq)]
pub enum Status {
    NotInstalled,
    /// set up and pointing at this program
    Ok,
    /// set up, but pointing at another program file (e.g. an older build)
    OtherProgram,
}

/// Is the right-click entry set up, and does it call this program?
pub fn status() -> Status {
    let Some(dir) = service_dir() else { return Status::NotInstalled };
    let Ok(doc) = std::fs::read_to_string(dir.join("Contents/document.wflow")) else {
        return Status::NotInstalled;
    };
    match std::env::current_exe() {
        Ok(exe) if doc.contains(&xml_escape(&exe.display().to_string())) => Status::Ok,
        _ => Status::OtherProgram,
    }
}

/// Entries of earlier versions that are still in the right-click menu.
pub fn old_entries() -> usize {
    old_service_dirs().len()
}

pub fn install() -> Result<PathBuf, String> {
    if !cfg!(target_os = "macos") {
        return Err("Der systemweite Rechtsklick-Dienst ist nur auf macOS verfügbar.".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = service_dir().ok_or("Kein Benutzerordner gefunden")?;
    let contents = dir.join("Contents");
    std::fs::create_dir_all(&contents).map_err(|e| e.to_string())?;
    let cmd = format!("\"{}\" --snippet --source \"Rechtsklick\"", exe.display());
    std::fs::write(contents.join("Info.plist"), info_plist()).map_err(|e| e.to_string())?;
    std::fs::write(contents.join("document.wflow"), wflow(&xml_escape(&cmd))).map_err(|e| e.to_string())?;
    // remove the entries of earlier versions; they point at program files that no longer exist
    for old in old_service_dirs() {
        let _ = std::fs::remove_dir_all(old);
    }
    // ask macOS to re-read the services menu (harmless if it fails)
    let _ = std::process::Command::new("/System/Library/CoreServices/pbs").arg("-update").output();
    Ok(dir)
}

fn info_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSServices</key>
	<array>
		<dict>
			<key>NSMenuItem</key>
			<dict>
				<key>default</key>
				<string>{SERVICE_NAME}</string>
			</dict>
			<key>NSMessage</key>
			<string>runWorkflowAsService</string>
			<key>NSSendTypes</key>
			<array>
				<string>public.utf8-plain-text</string>
			</array>
		</dict>
	</array>
</dict>
</plist>
"#
    )
}

fn wflow(cmd: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AMApplicationBuild</key>
	<string>523</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>{cmd}</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>0</integer>
					<key>shell</key>
					<string>/bin/bash</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>6A1B2C3D-0000-4000-8000-000000000001</string>
				<key>Keywords</key>
				<array>
					<string>Shell</string>
					<string>Script</string>
				</array>
				<key>OutputUUID</key>
				<string>6A1B2C3D-0000-4000-8000-000000000002</string>
				<key>UUID</key>
				<string>6A1B2C3D-0000-4000-8000-000000000003</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>arguments</key>
				<dict>
					<key>0</key>
					<dict>
						<key>default value</key>
						<integer>0</integer>
						<key>name</key>
						<string>inputMethod</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>0</string>
					</dict>
					<key>1</key>
					<dict>
						<key>default value</key>
						<string></string>
						<key>name</key>
						<string>source</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>1</string>
					</dict>
					<key>2</key>
					<dict>
						<key>default value</key>
						<false/>
						<key>name</key>
						<string>CheckedForUserDefaultShell</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>2</string>
					</dict>
					<key>3</key>
					<dict>
						<key>default value</key>
						<string></string>
						<key>name</key>
						<string>COMMAND_STRING</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>3</string>
					</dict>
					<key>4</key>
					<dict>
						<key>default value</key>
						<string>/bin/sh</string>
						<key>name</key>
						<string>shell</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>4</string>
					</dict>
				</dict>
				<key>isViewVisible</key>
				<integer>1</integer>
				<key>location</key>
				<string>309.000000:253.000000</string>
				<key>nibPath</key>
				<string>/System/Library/Automator/Run Shell Script.action/Contents/Resources/Base.lproj/main.nib</string>
			</dict>
			<key>isViewVisible</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>applicationBundleIDsByPath</key>
		<dict/>
		<key>applicationPaths</key>
		<array/>
		<key>inputTypeIdentifier</key>
		<string>com.apple.Automator.text</string>
		<key>outputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>presentationMode</key>
		<integer>11</integer>
		<key>processesInput</key>
		<false/>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.text</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<false/>
		<key>systemImageName</key>
		<string>NSActionTemplate</string>
		<key>useAutomaticInputType</key>
		<false/>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#
    )
}
