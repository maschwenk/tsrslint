# {{name}}

The `{{os}}-{{cpu}}` binary of [`{{mainName}}`](https://www.npmjs.com/package/{{mainName}}), a Rust port of
tsgolint (oxlint's type-aware lint backend) on top of tsrs. Do not install this package directly: install
`{{mainName}}`, which depends on it through `optionalDependencies`; `require("{{mainName}}").binaryPath()` returns
the path of the `tsrslint` binary in this package.

Version {{version}}, built against maschwenk/tsrs@{{tsrsCommit}}. MIT (see `LICENSE`); tsrs is Apache-2.0 (see
`THIRD-PARTY-LICENSE`).
