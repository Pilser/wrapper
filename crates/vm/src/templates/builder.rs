//! Builder for VM templates.

use core::VmTemplate;

pub struct VmTemplateBuilder {
    template: VmTemplate,
}

impl VmTemplateBuilder {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        let id = id.into();
        let name = name.into();
        Self {
            template: VmTemplate {
                id,
                name,
                description: None,
                rootfs_path: String::new(),
                kernel_path: String::new(),
                vcpu_count: 2,
                memory_mb: 2048,
                requires_gpu: false,
                status: core::VmTemplateStatus::Active,
                created_at: String::new(),
                updated_at: String::new(),
            },
        }
    }

    pub fn rootfs(mut self, path: impl Into<String>) -> Self {
        self.template.rootfs_path = path.into();
        self
    }

    pub fn kernel(mut self, path: impl Into<String>) -> Self {
        self.template.kernel_path = path.into();
        self
    }

    pub fn vcpu(mut self, n: u32) -> Self {
        self.template.vcpu_count = n;
        self
    }

    pub fn memory_mb(mut self, mb: u32) -> Self {
        self.template.memory_mb = mb;
        self
    }

    pub fn gpu(mut self, yes: bool) -> Self {
        self.template.requires_gpu = yes;
        self
    }

    pub fn build(self) -> VmTemplate {
        self.template
    }
}
