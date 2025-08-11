Template for blockscout Rust service
===

1. Install [cargo-generate](https://cargo-generate.github.io/cargo-generate/installation.html) using `cargo install cargo-generate`

2. Make sure you are in `blockscout-rs` directory and run

```bash
> cargo generate --path service-template
🤷   Project Name: test-project
🔧   Destination: /Users/levlymarenko/poa/blockscout-rs/test-project ...
🔧   project-name: test-project ...
🔧   Generating template ...
✔ 🤷   Include logic crate? · true
✔ 🤷   Does server need to connect to postgres database? · true
✔ 🤷   Generate github cicd YML file in .github/workflows? (You will need to move it to right location) · true
✔ 🤷   Generate example service, endpoints and messages in proto definition? · true
✔ 🤷   Generate typescript definitions for resources · true
🤷   Write example service name to generating proto examples [default: TestService]: TestService
✔ 🤷   Include entity crate inside logic crate? · true
✔ 🤷   Include migration crate inside logic crate? · true
🔧   Moving generated files into: `/Users/levlymarenko/poa/blockscout-rs/test-project`...
🔧   Initializing a fresh Git repository
✨   Done! New project created /Users/levlymarenko/poa/blockscout-rs/test-project
```

