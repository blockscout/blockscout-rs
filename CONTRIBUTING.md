Contributing to Blockscout-rs
===

First off, thanks for taking the time to contribute to 🚀 Blockscout-Rust 🚀 projects! 🎉

## How Can I Contribute?

The following is a set of guidelines for contributing to `blockscout-rs`, which is hosted in the Blockscout organization.

### Reporting Bugs

This section guides you through submitting a bug report. Following these guidelines helps maintainers and the community understand your report, reproduce the behavior, and find related reports.

#### Before Submitting a Bug Report

* Check the issues list to see if the problem has already been reported.
* Update to the latest version and see if the issue persists.


#### How Do I Submit a ✨Good✨ Bug Report?

Bugs are tracked as GitHub issues. Create an issue and provide the following information by filling in the provided template:

* Use a clear and descriptive title for the issue to identify the problem.
* Describe the exact steps which reproduce the problem in as many details as possible.
* Describe the behavior you observed after following the steps and explain why it is problematic.
* Explain which behavior you expected to see instead and why.
* Include screenshots, logs, ENVs, or any other information that might help in understanding the problem.
* Mark the issue with the appropriate label (bug, enhancement, documentation, question, etc).

### Contributing Code

If you want to help us improve the project, you can contribute code. To do so, follow these steps:

* Fork (https://github.com/blockscout/blockscout-rs/fork) the repository and make changes on your fork in a feature branch.
* Create your branch with the name `<name>/<short-slug-of-your-feature-or-fix>`
* Write code to implement features or fix bugs. Make sure your code is well-tested
* Commit your changes. Commit messages **SHOULD** follow our [commit message conventions](#commit-messages-guidelines) format.
* Create a pull request. The title of the pull request **MUST** follow the [commit message conventions](#commit-messages-guidelines) format.


### Commit Messages Guidelines

1. Use Conventional Commits:
   * Follow [Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/).

2. **Commit Types:**
   > **Note:** those types are preferred, but you can use any other type if it fits better.

   * `feat`: New feature
   * `fix`: Bug fix
   * `perf`: Performance improvement
   * `chore`: Maintenance tasks (including code changes that do not fit other types)
   * `refactor`: Code restructuring
   * `ci`: Continuous Integration changes
   * `docs`: Documentation updates
   * `build`: Build system changes
   * `config`: Configuration changes
   * `test`: Adding or updating tests
   * `revert`: Reverting commit changes

3. Message Format `type(scope): description`
   * **Type:** Lowercase (e.g., `feat`, `fix`)
   * **Scope:** Service name (optional)
   * **Description:** Short and concise (imperative mood)

4. **Examples:**
   * `feat: add new feature ...`
   * `fix: correct bug ...`
   * `docs: update README`
   * `ci: add new workflow`

5. Pull Request Titles

   * **SHOULD** include service name (in `lower-kebab-case`) or other scope of change after `<type>`
   * Examples:
     * `feat(stats): add resolution for newAccounts chart`
     * `fix(stats): correct wrong update bug`
     * `docs: add CONTRIBUTING.md`
     * `ci(verifier): update docker build step`

