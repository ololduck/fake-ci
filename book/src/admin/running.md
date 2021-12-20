# Running

## Watcher
For now, FakeCI only supports one mode: a repository watcher. It is provided in the binary, via the `watch` subcommand:

```text
$ fake-ci --help
fake-ci 0.1.0
Paul O.
A CI system written in rust

USAGE:
    fake-ci [OPTIONS] [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --config <FILE>    Sets a config file [default: fake-ci.toml]

SUBCOMMANDS:
    help     Prints this message or the help of the given subcommand(s)
    watch    Runs FakeCI in pulling mode; it will watch predefined repositories 
             and attempt to pull them
```

### Watcher Configuration

The watcher _needs_ a configuration file to work. Its configuration structure is the following:

```rust
pub struct FakeCIBinaryConfig {
    pub watch_interval: u32, // in seconds, defaults to 300
    pub repositories: Vec<FakeCIBinaryRepositoryConfig>, // an array of the following structure
}
pub struct FakeCIBinaryRepositoryConfig {
    pub name: String, // a human-readable name
    pub uri: String, // where we can find the git repo
    pub branches: BranchesSpec, // either a single branch or an array of branches
    pub notifiers: Option<Vec<Notifier>>, // an optional array of Notifiers. 
                                          // You should have at least one if you don't want to
                                          // spend your days glued to the output console
}
pub enum BranchesSpec {
    Single(String), // branches: main
    Multiple(Vec<String>), // branches:
                           // - main
                           // - feature/*
}
```

#### Sample configuration

Here's the config file I use to dog-feed:

```yaml
watch_interval: 30
repositories:
  - name: fakeci
    uri: "https://github.com/paulollivier/fake-ci.git"
    branches: "*"
    notifiers:
      - type: mailer
        config:
          from: Fake CI <fakeci@home.net>
          server: # a maildev instance
            addr: localhost 
            port: 1025
```

### Watcher gotchas

The watcher stores a cache of repository `refs` either in `$XDG_CACHE_DIR/fake-ci/` or `~/.cache/fake-ci/`. It will create json files with what it remembers as the commit hashes matching refs, as to be able to run only on changes.
