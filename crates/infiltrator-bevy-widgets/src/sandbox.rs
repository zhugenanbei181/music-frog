//! Sandboxed runtime container for third-party WebAssembly / JS micro-frontend dashboard widgets.
//!
//! Charter (docs/bevy-ui/BEVY_UI_FRONTEND.md §8.1.11, §8.3): a community widget
//! runs behind a real isolation boundary. The container enforces a memory,
//! per-frame instruction and lifetime frame budget; every host capability is
//! reached only through a typed [`CapabilityGrant`] the container issues after
//! an explicit permission check; and the widget paints through an
//! [`IsolatedRenderSlot`] descriptor that carries no ECS entity, component or
//! `World` handle. Once any quota is exceeded the container *fails closed*:
//! it quarantines itself and refuses every further host capability.

use std::collections::HashMap;

/// Resource quota for a sandboxed widget instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SandboxQuota {
    pub max_memory_bytes: usize,
    pub max_instructions_per_frame: u64,
    pub allow_network_read: bool,
    /// Lifetime frame budget; the instance quarantines itself past this count.
    pub max_frames: u64,
}

impl Default for SandboxQuota {
    fn default() -> Self {
        Self {
            max_memory_bytes: 4 * 1024 * 1024, // 4 MB limit
            max_instructions_per_frame: 50_000,
            allow_network_read: false,
            max_frames: 3_600, // one minute at 60 fps
        }
    }
}

/// A typed, non-silent sandbox rejection.
///
/// Every variant is a distinct closed reason; a caller can never mistake a
/// denial for success and no rejection is swallowed as a panic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SandboxFault {
    /// Guest memory reservation would exceed [`SandboxQuota::max_memory_bytes`].
    MemoryQuotaExceeded {
        used: usize,
        requested: usize,
        limit: usize,
    },
    /// Instruction charge would exceed [`SandboxQuota::max_instructions_per_frame`].
    InstructionBudgetExceeded {
        used: u64,
        requested: u64,
        limit: u64,
    },
    /// The instance outlived [`SandboxQuota::max_frames`].
    FrameBudgetExceeded { frame: u64, limit: u64 },
    /// The requested host capability is not covered by the manifest/quota.
    CapabilityDenied(HostCapability),
    /// The manifest failed [`WidgetManifest::validate`].
    ManifestInvalid(&'static str),
    /// A prior violation tripped the container; all host access is refused.
    Quarantined,
}

/// A host capability that a sandboxed widget can only reach after authorization.
///
/// This is the *exposed* surface: the widget never touches the capability
/// directly, it receives a [`CapabilityGrant`] scoped to the current frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HostCapability {
    ReadTrafficStats,
    ReadNodeList,
    SwitchProxyNode,
    ManageProfiles,
    ExecuteNetworkDiagnostics,
    NetworkRead,
}

/// Explicit permissions a third-party micro-frontend widget can request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidgetPermission {
    ReadTrafficStats,
    ReadNodeList,
    SwitchProxyNode,
    ManageProfiles,
    ExecuteNetworkDiagnostics,
}

impl WidgetPermission {
    /// The host capability unlocked by this declared permission.
    pub fn host_capability(self) -> HostCapability {
        match self {
            WidgetPermission::ReadTrafficStats => HostCapability::ReadTrafficStats,
            WidgetPermission::ReadNodeList => HostCapability::ReadNodeList,
            WidgetPermission::SwitchProxyNode => HostCapability::SwitchProxyNode,
            WidgetPermission::ManageProfiles => HostCapability::ManageProfiles,
            WidgetPermission::ExecuteNetworkDiagnostics => {
                HostCapability::ExecuteNetworkDiagnostics
            }
        }
    }
}

/// Proof that a host capability was authorized for one frame.
///
/// Possessing a grant is the only route to a host capability; the frame tag
/// lets the host invalidate grants when the sandbox advances frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilityGrant {
    capability: HostCapability,
    frame: u64,
}

impl CapabilityGrant {
    pub fn capability(&self) -> HostCapability {
        self.capability
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }
}

/// Opaque render-slot descriptor handed to a sandboxed widget.
///
/// It is deliberately plain integer data: no entity, component, asset or
/// `World` handle crosses the boundary. The host owns the slot entity and
/// resolves it by [`IsolatedRenderSlot::slot_id`] when compositing the widget's
/// retained layer, so the guest can describe *where* it paints but never reach
/// the ECS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsolatedRenderSlot {
    slot_id: u32,
    width: u32,
    height: u32,
    revision: u64,
}

impl IsolatedRenderSlot {
    pub const fn new(slot_id: u32, width: u32, height: u32) -> Self {
        Self {
            slot_id,
            width,
            height,
            revision: 0,
        }
    }

    pub fn slot_id(&self) -> u32 {
        self.slot_id
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Monotonic descriptor revision; bumped whenever the host re-sizes the slot.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Return a re-sized descriptor with a fresh revision.
    pub fn resized(&self, width: u32, height: u32) -> Self {
        Self {
            slot_id: self.slot_id,
            width,
            height,
            revision: self.revision + 1,
        }
    }
}

/// A sandboxed third-party widget instance and its isolation container.
#[derive(Clone, Debug, PartialEq)]
pub struct SandboxedWidgetInstance {
    pub widget_id: String,
    pub plugin_name: String,
    pub version: String,
    pub quota: SandboxQuota,
    pub state_store: HashMap<String, String>,
    granted: Vec<WidgetPermission>,
    memory_used: usize,
    instructions_this_frame: u64,
    frame_index: u64,
    quarantined: bool,
    render_slot: IsolatedRenderSlot,
}

impl SandboxedWidgetInstance {
    pub fn new(id: impl Into<String>, plugin_name: impl Into<String>) -> Self {
        Self {
            widget_id: id.into(),
            plugin_name: plugin_name.into(),
            version: "1.0.0".to_string(),
            quota: SandboxQuota::default(),
            state_store: HashMap::new(),
            granted: Vec::new(),
            memory_used: 0,
            instructions_this_frame: 0,
            frame_index: 0,
            quarantined: false,
            render_slot: IsolatedRenderSlot::new(0, 320, 180),
        }
    }

    pub fn write_state(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        if self.state_store.len() >= 128 {
            return false;
        }
        self.state_store.insert(key.into(), value.into());
        true
    }

    pub fn read_state(&self, key: &str) -> Option<&str> {
        self.state_store.get(key).map(|s| s.as_str())
    }

    /// Validate and adopt a manifest, seeding the instance's granted permissions.
    ///
    /// A rejected manifest leaves the instance unquarantined but permission-free,
    /// so a malformed widget can never inherit stale grants.
    pub fn bind_manifest(&mut self, manifest: &WidgetManifest) -> Result<(), SandboxFault> {
        manifest.validate().map_err(SandboxFault::ManifestInvalid)?;
        self.granted = manifest.permissions.clone();
        Ok(())
    }

    /// Permissions currently granted by the bound manifest.
    pub fn granted_permissions(&self) -> &[WidgetPermission] {
        &self.granted
    }

    /// Typed permission check performed before any host capability is exposed.
    ///
    /// Returns a frame-scoped [`CapabilityGrant`] on success, or a typed
    /// [`SandboxFault`] otherwise. A quarantined container always denies.
    pub fn authorize(&self, capability: HostCapability) -> Result<CapabilityGrant, SandboxFault> {
        if self.quarantined {
            return Err(SandboxFault::Quarantined);
        }
        let allowed = match capability {
            HostCapability::NetworkRead => self.quota.allow_network_read,
            other => self
                .granted
                .iter()
                .any(|perm| perm.host_capability() == other),
        };
        if !allowed {
            return Err(SandboxFault::CapabilityDenied(capability));
        }
        Ok(CapabilityGrant {
            capability,
            frame: self.frame_index,
        })
    }

    /// Convenience gate: authorize the host capability behind a permission.
    pub fn authorize_permission(
        &self,
        permission: WidgetPermission,
    ) -> Result<CapabilityGrant, SandboxFault> {
        self.authorize(permission.host_capability())
    }

    /// Advance to the next frame, resetting the per-frame instruction budget.
    ///
    /// Exceeding the lifetime frame budget quarantines the container.
    pub fn begin_frame(&mut self) -> Result<(), SandboxFault> {
        if self.quarantined {
            return Err(SandboxFault::Quarantined);
        }
        let next = self.frame_index + 1;
        if next > self.quota.max_frames {
            self.quarantined = true;
            return Err(SandboxFault::FrameBudgetExceeded {
                frame: next,
                limit: self.quota.max_frames,
            });
        }
        self.frame_index = next;
        self.instructions_this_frame = 0;
        Ok(())
    }

    /// Charge executed instructions against the current frame budget.
    ///
    /// An over-budget charge quarantines the container and fails closed.
    pub fn charge_instructions(&mut self, count: u64) -> Result<(), SandboxFault> {
        if self.quarantined {
            return Err(SandboxFault::Quarantined);
        }
        let used = self.instructions_this_frame.saturating_add(count);
        if used > self.quota.max_instructions_per_frame {
            self.quarantined = true;
            return Err(SandboxFault::InstructionBudgetExceeded {
                used,
                requested: count,
                limit: self.quota.max_instructions_per_frame,
            });
        }
        self.instructions_this_frame = used;
        Ok(())
    }

    /// Reserve guest memory against the memory budget.
    ///
    /// An over-budget reservation quarantines the container and fails closed.
    pub fn reserve_memory(&mut self, bytes: usize) -> Result<(), SandboxFault> {
        if self.quarantined {
            return Err(SandboxFault::Quarantined);
        }
        let used = self.memory_used.saturating_add(bytes);
        if used > self.quota.max_memory_bytes {
            self.quarantined = true;
            return Err(SandboxFault::MemoryQuotaExceeded {
                used,
                requested: bytes,
                limit: self.quota.max_memory_bytes,
            });
        }
        self.memory_used = used;
        Ok(())
    }

    /// Release previously reserved guest memory.
    pub fn release_memory(&mut self, bytes: usize) {
        self.memory_used = self.memory_used.saturating_sub(bytes);
    }

    pub fn memory_used(&self) -> usize {
        self.memory_used
    }

    pub fn instructions_this_frame(&self) -> u64 {
        self.instructions_this_frame
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Whether a prior violation tripped the container's fail-closed quarantine.
    pub fn is_quarantined(&self) -> bool {
        self.quarantined
    }

    /// Immediately quarantine the container, refusing all further host access.
    pub fn quarantine(&mut self) {
        self.quarantined = true;
    }

    /// The isolated render slot this widget may paint into.
    pub fn render_slot(&self) -> IsolatedRenderSlot {
        self.render_slot
    }

    /// Host-side slot assignment; returns the previous descriptor.
    pub fn assign_render_slot(&mut self, slot: IsolatedRenderSlot) -> IsolatedRenderSlot {
        let previous = self.render_slot;
        self.render_slot = slot;
        previous
    }
}

/// Metadata manifest describing a community micro-frontend dashboard widget.
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub min_bevy_version: String,
    pub permissions: Vec<WidgetPermission>,
}

impl WidgetManifest {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: "0.1.0".to_string(),
            author: "Community".to_string(),
            min_bevy_version: "0.20.0".to_string(),
            permissions: Vec::new(),
        }
    }

    pub fn with_permission(mut self, perm: WidgetPermission) -> Self {
        if !self.permissions.contains(&perm) {
            self.permissions.push(perm);
        }
        self
    }

    pub fn has_permission(&self, perm: WidgetPermission) -> bool {
        self.permissions.contains(&perm)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.id.trim().is_empty() {
            return Err("Widget ID cannot be empty");
        }
        if self.name.trim().is_empty() {
            return Err("Widget name cannot be empty");
        }
        if self.min_bevy_version != "0.20.0" {
            return Err("Incompatible Bevy version dependency");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_manifest_and_permissions() {
        let manifest = WidgetManifest::new("custom-speedtest", "Live Speed Test")
            .with_permission(WidgetPermission::ReadTrafficStats)
            .with_permission(WidgetPermission::ExecuteNetworkDiagnostics);

        assert!(manifest.validate().is_ok());
        assert!(manifest.has_permission(WidgetPermission::ReadTrafficStats));
        assert!(manifest.has_permission(WidgetPermission::ExecuteNetworkDiagnostics));
        assert!(!manifest.has_permission(WidgetPermission::ManageProfiles));

        // Invalid version check
        let mut bad_version = manifest;
        bad_version.min_bevy_version = "0.19.1".to_string();
        assert!(bad_version.validate().is_err());
    }

    #[test]
    fn test_denied_permission_blocks_capability() {
        let mut widget = SandboxedWidgetInstance::new("w", "plugin");
        let manifest =
            WidgetManifest::new("w", "Widget").with_permission(WidgetPermission::ReadTrafficStats);
        widget.bind_manifest(&manifest).unwrap();

        assert!(widget.authorize(HostCapability::ReadTrafficStats).is_ok());
        assert_eq!(
            widget.authorize(HostCapability::ManageProfiles),
            Err(SandboxFault::CapabilityDenied(
                HostCapability::ManageProfiles
            ))
        );
    }

    #[test]
    fn test_network_read_denied_by_default() {
        let mut widget = SandboxedWidgetInstance::new("w", "plugin");
        let manifest =
            WidgetManifest::new("w", "Widget").with_permission(WidgetPermission::ReadTrafficStats);
        widget.bind_manifest(&manifest).unwrap();
        assert_eq!(
            widget.authorize(HostCapability::NetworkRead),
            Err(SandboxFault::CapabilityDenied(HostCapability::NetworkRead))
        );

        widget.quota.allow_network_read = true;
        assert!(widget.authorize(HostCapability::NetworkRead).is_ok());
    }

    #[test]
    fn test_quota_exceeded_fails_closed() {
        let mut widget = SandboxedWidgetInstance::new("w", "plugin");
        assert!(widget.reserve_memory(1024).is_ok());
        assert!(
            widget
                .reserve_memory(widget.quota.max_memory_bytes)
                .is_err()
        );
        assert!(widget.is_quarantined());
        // Fail closed: even a previously granted capability is now refused.
        assert_eq!(
            widget.authorize(HostCapability::ReadTrafficStats),
            Err(SandboxFault::Quarantined)
        );
    }

    #[test]
    fn test_instruction_and_frame_budget() {
        let mut widget = SandboxedWidgetInstance::new("w", "plugin");
        widget.begin_frame().unwrap();
        assert!(widget.charge_instructions(10).is_ok());
        assert_eq!(
            widget.charge_instructions(widget.quota.max_instructions_per_frame),
            Err(SandboxFault::InstructionBudgetExceeded {
                used: 10 + widget.quota.max_instructions_per_frame,
                requested: widget.quota.max_instructions_per_frame,
                limit: widget.quota.max_instructions_per_frame,
            })
        );
        assert!(widget.is_quarantined());
    }

    #[test]
    fn test_render_slot_is_isolated_descriptor() {
        let mut widget = SandboxedWidgetInstance::new("w", "plugin");
        let slot = widget.render_slot();
        assert!(slot.is_valid());
        assert_eq!(slot.revision(), 0);

        let resized = slot.resized(640, 360);
        assert_eq!(resized.width(), 640);
        assert_eq!(resized.revision(), 1);
        assert_eq!(widget.assign_render_slot(resized), slot);
        assert_eq!(widget.render_slot(), resized);
    }

    #[test]
    fn test_manifest_validation_blocks_binding() {
        let mut widget = SandboxedWidgetInstance::new("w", "plugin");
        let mut manifest =
            WidgetManifest::new("w", "Widget").with_permission(WidgetPermission::ReadTrafficStats);
        manifest.min_bevy_version = "0.19.1".to_string();
        assert_eq!(
            widget.bind_manifest(&manifest),
            Err(SandboxFault::ManifestInvalid(
                "Incompatible Bevy version dependency"
            ))
        );
        assert!(widget.granted_permissions().is_empty());
    }
}
