//! Conformance of this repository's plugins to Vehicoule's plugin host (ADR 0002).
//!
//! Every component in the directory named by `VEHICOULE_PLUGINS`, which `scripts/build-plugins.sh` fills, is installed
//! in the real host: its size, its compilation, its imports, `describe` with every request refused, the manifest rules
//! and the capabilities matched against the exports (Vehicoule, `docs/plugin-host-spec.md`, sections 3 to 5). A plugin
//! the app would refuse fails here first. A missing or empty directory fails the test: it is never skipped.
//!
//! The budgets below are arbitrary and generous, like the host's own tests: the spec gives no value yet, and the app's
//! real budgets will come from measurements (section 9). Passing here does not show that a plugin fits them.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;

    use vehicoule_plugin_host::{
        ByteSize, Fuel, HostConfig, HttpBudget, HttpRequest, HttpResponse, Network, NetworkError,
        PluginHost, RequestLimits, Ticks,
    };

    /// A network that never opens a socket and records every request that reaches it. During install the host refuses
    /// every request before the network (section 3), so nothing may ever be recorded here.
    #[derive(Default)]
    struct NoNetwork {
        requests: Mutex<Vec<HttpRequest>>,
    }

    impl Network for NoNetwork {
        fn fetch(
            &self,
            request: &HttpRequest,
            _limits: RequestLimits,
        ) -> Result<HttpResponse, NetworkError> {
            self.requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(request.clone());
            Err(NetworkError::Transport(String::from(
                "no network in the conformance test",
            )))
        }
    }

    fn config() -> HostConfig {
        HostConfig::new(
            ByteSize::new(8 << 20),
            Fuel::new(50_000_000),
            // Never reached: nothing ticks this host.
            Ticks::new(10),
            ByteSize::new(32 << 20),
            ByteSize::new(64 << 10),
            HttpBudget::new(3, ByteSize::new(1 << 20), Duration::from_secs(10)),
        )
    }

    /// Every `.wasm` file of the directory named by `VEHICOULE_PLUGINS`, sorted.
    fn components() -> Vec<PathBuf> {
        let dir = std::env::var_os("VEHICOULE_PLUGINS")
            .expect("VEHICOULE_PLUGINS must name the output of scripts/build-plugins.sh");
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", Path::new(&dir).display()))
            .map(|entry| entry.expect("a directory entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "wasm"))
            .collect();
        paths.sort();
        paths
    }

    #[test]
    fn every_plugin_installs_under_its_own_id() {
        let paths = components();
        // At least the template: an empty directory means the build did not run, not that all is well.
        assert!(
            paths
                .iter()
                .any(|p| p.file_stem().is_some_and(|s| s == "template")),
            "no template.wasm among {paths:?}"
        );

        let network = Arc::new(NoNetwork::default());
        let host = PluginHost::new(config(), Arc::clone(&network) as Arc<dyn Network>)
            .expect("the host starts");
        for path in paths {
            let bytes = std::fs::read(&path).expect("the component is readable");
            let plugin = host
                .install(&bytes)
                .unwrap_or_else(|e| panic!("{} does not install: {e}", path.display()));
            let stem = path.file_stem().and_then(|s| s.to_str());
            assert_eq!(
                Some(plugin.manifest().id().as_str()),
                stem,
                "{}: the manifest id must be the file name, which is the crate's directory",
                path.display()
            );
        }
        assert_eq!(
            network
                .requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .len(),
            0,
            "a request reached the network during install"
        );
    }
}
