# FakeCI

I decided to create this on a windy and rainy Sunday, while looking at the available Free & Open-Source CI & CD softwares.
I noticed there wasn't any written in rust, and I thought it would be a cool project to pass the time and hopefully one day have a cool solution.

This is really the very beginning of this project, there's no http interface, no docker support… no nothing, in fact :D

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

## Design

- Everything as flat files or envvars
- As much self-contained as possible.
- Support for multiple http hooks systems (priorities: GitHub, Gitea & GitLab)

## Plan

- [ ] Export build logs & artifacts
- [ ] HTTP hooks APIs
- [ ] Jobs execution in docker images (dir sharing amongst instances?) => Self-Cleanup
- [ ] Artifacts upload mechanisms
- [ ] Async exec, run jobs in parallel

### Nice to have

- [ ] jobs depends_on
- [ ] Web UI?