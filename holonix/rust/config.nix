rec {
  # the target used by rust when compiling wasm
  wasm-target = "wasm32-unknown-unknown";

  # the target used by all linux when we don't have a specific target
  generic-linux-target = "x86_64-unknown-linux-gnu";

  # the target used by all mac
  generic-mac-target = "x86_64-apple-darwin";

  # set this to "info" to debug compiler cache misses due to fingerprinting
  # @see https://github.com/rust-lang/cargo/issues/4961#issuecomment-359189913
  log = "warnings";

  # set this to "1" or "full" for rust backtraces
  # this is on because we assume you are developing in the shell
  backtrace = "1";

  compile = {

    # @see https://github.com/rust-unofficial/patterns/blob/master/anti_patterns/deny-warnings.md
    deny = "warnings";

    lto = "thinlto";

    # significantly improves cache hit rate when recompiling
    # much more reliable than default timestamp based compiler caching
    # often (e.g. on CI/windows) we lose timestamp info from the OS
    # achieves cache hits on freshly downloaded rust crates!
    # highly sensitive to changes in compiler environment variables
    # incompatible with some lto options
    incremental = "1";

    # the compiler will split each file into this many chunks and process
    # each in parallel.
    # compilation process is faster with more units but diminishing returns
    # final output supports fewer optimisations with additional units
    # @see https://www.ncameron.org/blog/how-fast-can-i-build-rust/
    codegen-units = "10";

    # the compiler may run this many parallel jobs
    # no real downside of increasing
    # has no additional effect past some point ~6
    # @see https://www.ncameron.org/blog/how-fast-can-i-build-rust/
    # @see NUM_JOBS
    # @see https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts
    jobs = "6";

    # 0 = none
    # 1 = less
    # 2 = default
    # 3 = aggressive
    # s = size
    # z = size min
    #optimization-level = "z";

    # TODO: see how many of these we can enable on stable today
    # @see https://llogiq.github.io/2017/06/01/perf-pitfalls.html
    # flags ="-D ${base.compile.deny} -Z external-macro-backtrace -Z ${base.compile.lto} -C codegen-units=${base.compile.codegen-units}";
    stable-flags =
      "-D ${compile.deny} -C codegen-units=${compile.codegen-units}";
  };

  test = {
    # test threads can be the same as top level build parallelization
    threads = compile.jobs;
  };
}
