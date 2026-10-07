//! A host injects one script service pair shared with its reader.
use super::CommandApplication;
use crate::script_application::ScriptApplication;
use crate::script_export_application::ScriptExportApplication;

impl CommandApplication {
    pub fn with_scripts(
        mut self,
        scripts: ScriptApplication,
        exports: ScriptExportApplication,
    ) -> Self {
        self.scripts = Some(scripts);
        self.script_exports = Some(exports);
        self
    }
}
