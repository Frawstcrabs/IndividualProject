This project can be assembled together using Rust's Cargo binary. Open a terminal in this directly, and type "cargo run" to compile and run the program. Alternatively, one can use the binary named "interpreter" found in the root of the source project files to execute code. This can be done via "interpreter examples/mandelbrot.txt", for example.

To run one of the files found in examples or lang_tests, use "cargo run -- filename", e.g. "cargo run -- examples/mandelbrot.txt".

Arguments can be passed to the file as follows: "cargo run -- examples/enigma.txt TESTINGSTRING"

One can also run code directly on the command line without having to save to file first. This can be achieved with the following: "cargo run -- -c "{args[0]}" test".

To run the test suite for this language, simply run "cargo test". The screen will show a list of all the tests that were run, a status of their success, and the way in which they failed if any have. It is important to note that, due to the fact that the test framework runs the compiled binary directly to execute tests, the binary must be present within the "target/debug/individual_project" folder. This is the default location for the binary when "cargo build" or "cargo run" is used.

To enable optimisations for the code, one can add "--release" to the command, e.g. "cargo run --release -- examples/mandelbrot.txt".

Due to the way the libgc library compiles the Boehm GC for Rust, one can only build and run this project within a Linux system that has a C compiler and common development tools installed, such as the "base-devel" package on Arch, or other similar packages on other Linux distributions.