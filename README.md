# FakeCI

I decided to create this on a windy and rainy Sunday, while looking at the available Free & Open-Source CI & CD
softwares. I noticed there wasn't any written in rust, and I thought it would be a cool project to pass the time and
hopefully one day have a cool solution.

This is really the very beginning of this project, there's no http interface, … no nothing, in fact :D

The only redeeming feature is it's ability to build & test itself, for now. That's why it's called FakeCI.

```log
 INFO  fakeci > Running job "hello world"
 INFO  fakeci >  Running step "Create File"
 INFO  fakeci >   - touch /tmp/hello_world
test tests::test_hello_world ... ok
 INFO  fakeci > Running job "check, test & release"
 INFO  fakeci >  Running step "check"
 INFO  fakeci >   - cargo check
 DEBUG fakeci >     stderr:     Updating crates.io index
 DEBUG fakeci >     stderr:    Compiling proc-macro2 v1.0.32
# ---- 8< ----
 DEBUG fakeci >     stderr:     Checking fake-ci v0.1.0 (/tmp/fakeci_execution.PpbYI831jrPk)
 DEBUG fakeci >     stderr:     Finished dev [unoptimized + debuginfo] target(s) in 8.12s
 INFO  fakeci >  Running step "test"
 INFO  fakeci >   - cargo test test_hello_world
 DEBUG fakeci >     stdout:
 DEBUG fakeci >     stdout: running 1 test
 DEBUG fakeci >     stdout: test tests::test_hello_world ... ok
 DEBUG fakeci >     stdout:
 DEBUG fakeci >     stdout: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
 DEBUG fakeci >     stdout:
 DEBUG fakeci >     stdout:
 DEBUG fakeci >     stdout: running 0 tests
 DEBUG fakeci >     stdout:
 DEBUG fakeci >     stdout: test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
 DEBUG fakeci >     stdout:
 DEBUG fakeci >     stderr:    Compiling regex-syntax v0.6.25
 DEBUG fakeci >     stderr:    Compiling quick-error v1.2.3
# ---- 8< ----
 DEBUG fakeci >     stderr:    Compiling fake-ci v0.1.0 (/tmp/fakeci_execution.PpbYI831jrPk)
 DEBUG fakeci >     stderr:     Finished test [unoptimized + debuginfo] target(s) in 2.67s
 DEBUG fakeci >     stderr:      Running unittests (target/debug/deps/fakeci-f13573da498c3f66)
 DEBUG fakeci >     stderr:  INFO  fakeci > Running job "hello world"
 DEBUG fakeci >     stderr:  INFO  fakeci >  Running step "Create File"
 DEBUG fakeci >     stderr:  INFO  fakeci >   - touch /tmp/hello_world
 DEBUG fakeci >     stderr:      Running unittests (target/debug/deps/fake_ci-fbd39b80884cbe2f)
 INFO  fakeci >  Running step "release"
 INFO  fakeci >   - cargo build --release
 DEBUG fakeci >     stderr:    Compiling proc-macro2 v1.0.32
 DEBUG fakeci >     stderr:    Compiling libc v0.2.108
 DEBUG fakeci >     stderr:    Compiling unicode-xid v0.2.2
# ---- 8< ----
 DEBUG fakeci >     stderr:    Compiling fake-ci v0.1.0 (/tmp/fakeci_execution.PpbYI831jrPk)
 DEBUG fakeci >     stderr:     Finished release [optimized] target(s) in 6.30s
test tests::test_dog_feeding ... ok
```

## Repository configuration

A `.fakeci.yml` file needs to be placed at the repository's root. You will find an example thereafter:

```yaml
---
# optional: Some defaults can be set that will apply to the whole pipeline, unless overridden
default:
    # optional: we use docker to run stuff into, so here we define the rust image
    image: rust

# a "pipeline" is a collection of "jobs", themselves comprising of "steps", containing "commands"
pipeline:
    # this is a job definition
    - name: check, test & release # it has a name
      # NOTE: either a per-job or a default image definition is needed
      image: # optional: long form of using an image from docker hub
          name: rust # we can specify the image here (not needed in this example, as we defined it in defaults
          privileged: false # by default
      # NOTE: a job uses a single, re-used container
      env: # optional: we can define envvars to pass to the container
          RUST_LOG: debug
      # optional: a list of volumes to mount.
      # NOTE: the repository will always be mounted as /code in the container.
      volumes:
          # let's share the build cache between jobs by using a named volume (not yet implemented)
          - fake-ci-target:/code/target
      steps:
          # a "step" is:
          - name: check # a name, used to identify the step in the log. If not given, "step {n}" is used
            exec: # a list of commands to execute
                - cargo check
          - name: test
            exec:
                - cargo test test_hello_world
          - name: release
            exec:
                - cargo build --release
    - name: Run my special software
      image: # let's tell Fake CI to build & use our own image, built from dockerfile
          dockerfile: resources/mysoft/Dockerfile # optional: will be Dockerfile by default
          context: resources/mysoft # optional: change build context
          build_args: HTTP_PROXY # optional: sets build-time vars
          name: mysoft:latest # optional: give it a custom tag
          privileged: false # optional: runs in privileged mode
      steps:
          - name: run mysoft
            exec:
                - mysoft
```

## Installation

For now, `git clone` this repo. Maybe then you can `cargo install --path .` it.

## Running

We now have an event-loop-based binary! Here's its help page:

```
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
    watch    Runs FakeCI in pulling mode; it will watch predefined repositories and attempt to pull them
```

As you can see, the `watch` subcommands wants for a configuration file. Here's an example:

```yaml
watch_interval: 300 # timer on the event loop, in seconds
repositories: # list of repositories
    - name: fake-ci # arbitrary name
      uri: https://github.com/paulollivier/fake-ci.git
      # alternate form, for instance for a gitflow branching model
      # branches:
      #   - main
      #   - feature/*
      #   - hotfix/*
      branches: "*" # watch all branches matching this glob expression
      notifiers: # notifiers control how to be notified of build results
          - type: mailer # for now, only the "mailer" type is available
    - config:
          from: Fake CI <fakeci@home.net> # From: address
          server: # SMTP server to connect to. Here, a maildev.
              addr: localhost
              port: 1025
              # NOTE: for now, this config can't use SMTP auth or SSL connections
```

## Design

-   Everything as flat files or envvars
-   As much self-contained as possible.
-   Support for multiple http hooks systems (priorities: GitHub, Gitea & GitLab)

## Plan

-   [ ] Export build logs & artifacts
-   [ ] HTTP hooks APIs
-   [ ] Jobs execution in docker images (dir sharing amongst instances?) => Self-Cleanup
-   [ ] Artifacts upload mechanisms
-   [ ] Async exec, run jobs in parallel

### Nice to have

-   [ ] jobs depends_on
-   [ ] Web UI?
