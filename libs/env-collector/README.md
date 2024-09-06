Env collector
===

This is a simple tool to collect possible environment variables from `Settings` structure and place it to `README.md` file.

## Usage

1. Add `env-collector` to your `server` crate:
    ```toml
    # Cargo.toml
    [dependencies]
    env-collector = { git = "https://github.com/blockscout/blockscout-rs", version = "0.1.1" }
    ```

2. In your `server` crate create new binary file called `check-envs.rs` with the following content:

    ```rust
    // check-envs.rs
    use <path_to_settings>::Settings;
    use env_collector::run_env_collector_cli;
    
    fn main() {
        run_env_collector_cli::<Settings>(
            "<SERVICE_NAME_PREFIX>",
            "README.md",
            "<PATH TO .TOML/.JSON EXAMPLE CONFIG>",
            &[PrefixFilter::blacklist("<ENV_PREFIX_TO_IGNORE>")],
            Some("some_postfix"),
        );
    }
    ```
3. In `README.md` file add special **anchors** lines to specify where to store the table with ENVs:

    ```markdown
    ## Envs

    [anchor]: <> (anchors.envs.start.some_postfix)
    [anchor]: <> (anchors.envs.end.some_postfix)
    ```

4. (Optional) In your `justfile` add new command to run `check-envs.rs`:

    ```just
    # justfile
    check-envs:
        cargo run --bin check-envs
    ```

5. Run command `just check-envs` to generate ENVs table in `README.md` file.

6. Add github action to run this binary on every push to validate ENVs table in `README.md` file:    
    ```yaml
    [... other steps of `test` job ...]
    
    - name: ENVs in doc tests
      run: cargo run --bin check-envs
      env:
        VALIDATE_ONLY: true
    
    [... other steps of `test` job ...]
      ```