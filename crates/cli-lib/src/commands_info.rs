pub(crate) fn info() {
    // Print information about the current environment
    println!("Rust Version: {}", env!("CARGO_PKG_VERSION"));

    // Print information about python
    #[cfg(feature = "python")]
    {
        use pyo3::prelude::*;
        use pyo3::types::{PyList, PyMapping, PyString};

        // Acquire the Global Interpreter Lock (GIL) and open a Python context
        Python::attach(|py| {
            // Import the sys module
            let sys = py.import("sys").unwrap();

            // Get some attributes from sys
            let version = sys.getattr("version").unwrap();
            let executable = sys.getattr("executable").unwrap();
            let prefix = sys.getattr("prefix").unwrap();
            let base_prefix = sys.getattr("base_prefix").unwrap();
            // Print them out or do whatever you want with them
            println!("Python Version: {}", version.str().unwrap());
            println!("Executable: {}", executable.str().unwrap());
            println!("Prefix: {}", prefix.str().unwrap());
            println!("Base Prefix: {}", base_prefix.str().unwrap());

            let sys_path = sys.getattr("path").unwrap();
            let sys_path: &Bound<'_, PyList> = sys_path.cast().unwrap();
            println!("sys.path:");
            for p in sys_path.iter() {
                println!("  {p}");
            }

            // If you need to check environment variables (e.g. VIRTUAL_ENV),
            // you can import "os" and query os.environ:
            let os = py.import("os").unwrap();
            let environ = os.getattr("environ").unwrap();
            let environ: &Bound<'_, PyMapping> = environ.cast().unwrap();
            // If VIRTUAL_ENV is set, you can get it like:
            if let Ok(virtual_env_val) = environ.get_item("VIRTUAL_ENV") {
                let virtual_env: Result<&Bound<'_, PyString>, _> = virtual_env_val.cast();
                if let Ok(virtual_env) = virtual_env {
                    println!("VIRTUAL_ENV: {virtual_env}");
                } else {
                    println!("VIRTUAL_ENV not set.");
                }
            } else {
                println!("VIRTUAL_ENV not set.");
            }

            Ok::<_, String>(())
        })
        .unwrap();
    }
}
