# This file was @generated by cargo2nix 0.11.0.
# It is not intended to be manually edited.

args@{
  release ? true,
  rootFeatures ? [
    "webschembly-compiler/default"
    "webschembly-compiler-cli/default"
    "webschembly-runtime/default"
  ],
  rustPackages,
  buildRustPackages,
  hostPlatform,
  hostPlatformCpu ? null,
  hostPlatformFeatures ? [],
  target ? null,
  codegenOpts ? null,
  profileOpts ? null,
  cargoUnstableFlags ? null,
  rustcLinkFlags ? null,
  rustcBuildFlags ? null,
  mkRustCrate,
  rustLib,
  lib,
  workspaceSrc,
  ignoreLockHash,
}:
let
  nixifiedLockHash = "7bf8c06ee320e03f95c3f7009bfd5622bf5282d64ea76b80f526e7cb4db62c26";
  workspaceSrc = if args.workspaceSrc == null then ./. else args.workspaceSrc;
  currentLockHash = builtins.hashFile "sha256" (workspaceSrc + /Cargo.lock);
  lockHashIgnored = if ignoreLockHash
                  then builtins.trace "Ignoring lock hash" ignoreLockHash
                  else ignoreLockHash;
in if !lockHashIgnored && (nixifiedLockHash != currentLockHash) then
  throw ("Cargo.nix ${nixifiedLockHash} is out of sync with Cargo.lock ${currentLockHash}")
else let
  inherit (rustLib) fetchCratesIo fetchCrateLocal fetchCrateGit fetchCrateAlternativeRegistry expandFeatures decideProfile genDrvsByProfile;
  profilesByName = {
  };
  rootFeatures' = expandFeatures rootFeatures;
  overridableMkRustCrate = f:
    let
      drvs = genDrvsByProfile profilesByName ({ profile, profileName }: mkRustCrate ({ inherit release profile hostPlatformCpu hostPlatformFeatures target profileOpts codegenOpts cargoUnstableFlags rustcLinkFlags rustcBuildFlags; } // (f profileName)));
    in { compileMode ? null, profileName ? decideProfile compileMode release }:
      let drv = drvs.${profileName}; in if compileMode == null then drv else drv.override { inherit compileMode; };
in
{
  cargo2nixVersion = "0.11.0";
  workspace = {
    webschembly-compiler = rustPackages.unknown.webschembly-compiler."0.1.0";
    webschembly-compiler-cli = rustPackages.unknown.webschembly-compiler-cli."0.1.0";
    webschembly-runtime = rustPackages.unknown.webschembly-runtime."0.1.0";
  };
  "registry+https://github.com/rust-lang/crates.io-index".anstream."0.6.18" = overridableMkRustCrate (profileName: rec {
    name = "anstream";
    version = "0.6.18";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "8acc5369981196006228e28809f761875c0327210a891e941f4c683b3a99529b"; };
    features = builtins.concatLists [
      [ "auto" ]
      [ "default" ]
      [ "wincon" ]
    ];
    dependencies = {
      anstyle = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".anstyle."1.0.10" { inherit profileName; }).out;
      anstyle_parse = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".anstyle-parse."0.2.6" { inherit profileName; }).out;
      anstyle_query = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".anstyle-query."1.1.2" { inherit profileName; }).out;
      ${ if hostPlatform.isWindows then "anstyle_wincon" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".anstyle-wincon."3.0.7" { inherit profileName; }).out;
      colorchoice = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".colorchoice."1.0.3" { inherit profileName; }).out;
      is_terminal_polyfill = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".is_terminal_polyfill."1.70.1" { inherit profileName; }).out;
      utf8parse = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".utf8parse."0.2.2" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".anstyle."1.0.10" = overridableMkRustCrate (profileName: rec {
    name = "anstyle";
    version = "1.0.10";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "55cc3b69f167a1ef2e161439aa98aed94e6028e5f9a59be9a6ffb47aef1651f9"; };
    features = builtins.concatLists [
      [ "default" ]
      [ "std" ]
    ];
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".anstyle-parse."0.2.6" = overridableMkRustCrate (profileName: rec {
    name = "anstyle-parse";
    version = "0.2.6";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "3b2d16507662817a6a20a9ea92df6652ee4f94f914589377d69f3b21bc5798a9"; };
    features = builtins.concatLists [
      [ "default" ]
      [ "utf8" ]
    ];
    dependencies = {
      utf8parse = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".utf8parse."0.2.2" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".anstyle-query."1.1.2" = overridableMkRustCrate (profileName: rec {
    name = "anstyle-query";
    version = "1.1.2";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "79947af37f4177cfead1110013d678905c37501914fba0efea834c3fe9a8d60c"; };
    dependencies = {
      ${ if hostPlatform.isWindows then "windows_sys" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".windows-sys."0.59.0" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".anstyle-wincon."3.0.7" = overridableMkRustCrate (profileName: rec {
    name = "anstyle-wincon";
    version = "3.0.7";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "ca3534e77181a9cc07539ad51f2141fe32f6c3ffd4df76db8ad92346b003ae4e"; };
    dependencies = {
      anstyle = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".anstyle."1.0.10" { inherit profileName; }).out;
      ${ if hostPlatform.isWindows then "once_cell" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".once_cell."1.20.3" { inherit profileName; }).out;
      ${ if hostPlatform.isWindows then "windows_sys" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".windows-sys."0.59.0" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".anyhow."1.0.96" = overridableMkRustCrate (profileName: rec {
    name = "anyhow";
    version = "1.0.96";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "6b964d184e89d9b6b67dd2715bc8e74cf3107fb2b529990c90cf517326150bf4"; };
    features = builtins.concatLists [
      [ "default" ]
      [ "std" ]
    ];
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".bitflags."2.8.0" = overridableMkRustCrate (profileName: rec {
    name = "bitflags";
    version = "2.8.0";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "8f68f53c83ab957f72c32642f3868eec03eb974d1fb82e453128456482613d36"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".clap."4.5.30" = overridableMkRustCrate (profileName: rec {
    name = "clap";
    version = "4.5.30";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "92b7b18d71fad5313a1e320fa9897994228ce274b60faa4d694fe0ea89cd9e6d"; };
    features = builtins.concatLists [
      [ "color" ]
      [ "default" ]
      [ "derive" ]
      [ "error-context" ]
      [ "help" ]
      [ "std" ]
      [ "suggestions" ]
      [ "usage" ]
    ];
    dependencies = {
      clap_builder = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".clap_builder."4.5.30" { inherit profileName; }).out;
      clap_derive = (buildRustPackages."registry+https://github.com/rust-lang/crates.io-index".clap_derive."4.5.28" { profileName = "__noProfile"; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".clap_builder."4.5.30" = overridableMkRustCrate (profileName: rec {
    name = "clap_builder";
    version = "4.5.30";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "a35db2071778a7344791a4fb4f95308b5673d219dee3ae348b86642574ecc90c"; };
    features = builtins.concatLists [
      [ "color" ]
      [ "error-context" ]
      [ "help" ]
      [ "std" ]
      [ "suggestions" ]
      [ "usage" ]
    ];
    dependencies = {
      anstream = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".anstream."0.6.18" { inherit profileName; }).out;
      anstyle = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".anstyle."1.0.10" { inherit profileName; }).out;
      clap_lex = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".clap_lex."0.7.4" { inherit profileName; }).out;
      strsim = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".strsim."0.11.1" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".clap_derive."4.5.28" = overridableMkRustCrate (profileName: rec {
    name = "clap_derive";
    version = "4.5.28";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "bf4ced95c6f4a675af3da73304b9ac4ed991640c36374e4b46795c49e17cf1ed"; };
    features = builtins.concatLists [
      [ "default" ]
    ];
    dependencies = {
      heck = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".heck."0.5.0" { inherit profileName; }).out;
      proc_macro2 = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".proc-macro2."1.0.93" { inherit profileName; }).out;
      quote = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".quote."1.0.38" { inherit profileName; }).out;
      syn = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".syn."2.0.98" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".clap_lex."0.7.4" = overridableMkRustCrate (profileName: rec {
    name = "clap_lex";
    version = "0.7.4";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "f46ad14479a25103f283c0f10005961cf086d8dc42205bb44c46ac563475dca6"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".colorchoice."1.0.3" = overridableMkRustCrate (profileName: rec {
    name = "colorchoice";
    version = "1.0.3";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "5b63caa9aa9397e2d9480a9b13673856c78d8ac123288526c37d7839f2a86990"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".equivalent."1.0.2" = overridableMkRustCrate (profileName: rec {
    name = "equivalent";
    version = "1.0.2";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "877a4ace8713b0bcf2a4e7eec82529c029f1d0619886d18145fea96c3ffe5c0f"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".hashbrown."0.15.2" = overridableMkRustCrate (profileName: rec {
    name = "hashbrown";
    version = "0.15.2";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "bf151400ff0baff5465007dd2f3e717f3fe502074ca563069ce3a6629d07b289"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".heck."0.5.0" = overridableMkRustCrate (profileName: rec {
    name = "heck";
    version = "0.5.0";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "2304e00983f87ffb38b55b444b5e3b60a884b5d30c0fca7d82fe33449bbe55ea"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".indexmap."2.7.1" = overridableMkRustCrate (profileName: rec {
    name = "indexmap";
    version = "2.7.1";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "8c9c992b02b5b4c94ea26e32fe5bccb7aa7d9f390ab5c1221ff895bc7ea8b652"; };
    features = builtins.concatLists [
      [ "std" ]
    ];
    dependencies = {
      equivalent = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".equivalent."1.0.2" { inherit profileName; }).out;
      hashbrown = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".hashbrown."0.15.2" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".is_terminal_polyfill."1.70.1" = overridableMkRustCrate (profileName: rec {
    name = "is_terminal_polyfill";
    version = "1.70.1";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "7943c866cc5cd64cbc25b2e01621d07fa8eb2a1a23160ee81ce38704e97b8ecf"; };
    features = builtins.concatLists [
      [ "default" ]
    ];
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".leb128."0.2.5" = overridableMkRustCrate (profileName: rec {
    name = "leb128";
    version = "0.2.5";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "884e2677b40cc8c339eaefcb701c32ef1fd2493d71118dc0ca4b6a736c93bd67"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".log."0.4.26" = overridableMkRustCrate (profileName: rec {
    name = "log";
    version = "0.4.26";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "30bde2b3dc3671ae49d8e2e9f044c7c005836e7a023ee57cffa25ab82764bb9e"; };
    features = builtins.concatLists [
      [ "release_max_level_warn" ]
    ];
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".memchr."2.7.4" = overridableMkRustCrate (profileName: rec {
    name = "memchr";
    version = "2.7.4";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "78ca9ab1a0babb1e7d5695e3530886289c18cf2f87ec19a575a0abdce112e3a3"; };
    features = builtins.concatLists [
      [ "alloc" ]
      [ "std" ]
    ];
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".minimal-lexical."0.2.1" = overridableMkRustCrate (profileName: rec {
    name = "minimal-lexical";
    version = "0.2.1";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "68354c5c6bd36d73ff3feceb05efa59b6acb7626617f4962be322a825e61f79a"; };
    features = builtins.concatLists [
      [ "std" ]
    ];
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".nom."7.1.3" = overridableMkRustCrate (profileName: rec {
    name = "nom";
    version = "7.1.3";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "d273983c5a657a70a3e8f2a01329822f3b8c8172b73826411a55751e404a0a4a"; };
    features = builtins.concatLists [
      [ "alloc" ]
      [ "default" ]
      [ "std" ]
    ];
    dependencies = {
      memchr = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".memchr."2.7.4" { inherit profileName; }).out;
      minimal_lexical = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".minimal-lexical."0.2.1" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".once_cell."1.20.3" = overridableMkRustCrate (profileName: rec {
    name = "once_cell";
    version = "1.20.3";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "945462a4b81e43c4e3ba96bd7b49d834c6f61198356aa858733bc4acf3cbe62e"; };
    features = builtins.concatLists [
      [ "alloc" ]
      [ "default" ]
      [ "race" ]
      [ "std" ]
    ];
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".proc-macro2."1.0.93" = overridableMkRustCrate (profileName: rec {
    name = "proc-macro2";
    version = "1.0.93";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "60946a68e5f9d28b0dc1c21bb8a97ee7d018a8b322fa57838ba31cc878e22d99"; };
    features = builtins.concatLists [
      [ "default" ]
      [ "proc-macro" ]
    ];
    dependencies = {
      unicode_ident = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".unicode-ident."1.0.17" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".quote."1.0.38" = overridableMkRustCrate (profileName: rec {
    name = "quote";
    version = "1.0.38";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "0e4dccaaaf89514f546c693ddc140f729f958c247918a13380cccc6078391acc"; };
    features = builtins.concatLists [
      [ "default" ]
      [ "proc-macro" ]
    ];
    dependencies = {
      proc_macro2 = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".proc-macro2."1.0.93" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".rustversion."1.0.19" = overridableMkRustCrate (profileName: rec {
    name = "rustversion";
    version = "1.0.19";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "f7c45b9784283f1b2e7fb61b42047c2fd678ef0960d4f6f1eba131594cc369d4"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".semver."1.0.25" = overridableMkRustCrate (profileName: rec {
    name = "semver";
    version = "1.0.25";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "f79dfe2d285b0488816f30e700a7438c5a73d816b5b7d3ac72fbc48b0d185e03"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".strsim."0.11.1" = overridableMkRustCrate (profileName: rec {
    name = "strsim";
    version = "0.11.1";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "7da8b5736845d9f2fcb837ea5d9e2628564b3b043a70948a3f0b778838c5fb4f"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".strum."0.27.1" = overridableMkRustCrate (profileName: rec {
    name = "strum";
    version = "0.27.1";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "f64def088c51c9510a8579e3c5d67c65349dcf755e5479ad3d010aa6454e2c32"; };
    features = builtins.concatLists [
      [ "default" ]
      [ "std" ]
    ];
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".strum_macros."0.27.1" = overridableMkRustCrate (profileName: rec {
    name = "strum_macros";
    version = "0.27.1";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "c77a8c5abcaf0f9ce05d62342b7d298c346515365c36b673df4ebe3ced01fde8"; };
    dependencies = {
      heck = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".heck."0.5.0" { inherit profileName; }).out;
      proc_macro2 = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".proc-macro2."1.0.93" { inherit profileName; }).out;
      quote = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".quote."1.0.38" { inherit profileName; }).out;
      rustversion = (buildRustPackages."registry+https://github.com/rust-lang/crates.io-index".rustversion."1.0.19" { profileName = "__noProfile"; }).out;
      syn = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".syn."2.0.98" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".syn."2.0.98" = overridableMkRustCrate (profileName: rec {
    name = "syn";
    version = "2.0.98";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "36147f1a48ae0ec2b5b3bc5b537d267457555a10dc06f3dbc8cb11ba3006d3b1"; };
    features = builtins.concatLists [
      [ "clone-impls" ]
      [ "default" ]
      [ "derive" ]
      [ "full" ]
      [ "parsing" ]
      [ "printing" ]
      [ "proc-macro" ]
    ];
    dependencies = {
      proc_macro2 = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".proc-macro2."1.0.93" { inherit profileName; }).out;
      quote = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".quote."1.0.38" { inherit profileName; }).out;
      unicode_ident = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".unicode-ident."1.0.17" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".unicode-ident."1.0.17" = overridableMkRustCrate (profileName: rec {
    name = "unicode-ident";
    version = "1.0.17";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "00e2473a93778eb0bad35909dff6a10d28e63f792f16ed15e404fca9d5eeedbe"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".utf8parse."0.2.2" = overridableMkRustCrate (profileName: rec {
    name = "utf8parse";
    version = "0.2.2";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "06abde3611657adf66d383f00b093d7faecc7fa57071cce2578660c9f1010821"; };
    features = builtins.concatLists [
      [ "default" ]
    ];
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".wasm-encoder."0.221.3" = overridableMkRustCrate (profileName: rec {
    name = "wasm-encoder";
    version = "0.221.3";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "dc8444fe4920de80a4fe5ab564fff2ae58b6b73166b89751f8c6c93509da32e5"; };
    features = builtins.concatLists [
      [ "component-model" ]
      [ "default" ]
    ];
    dependencies = {
      leb128 = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".leb128."0.2.5" { inherit profileName; }).out;
      wasmparser = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".wasmparser."0.221.3" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".wasmparser."0.221.3" = overridableMkRustCrate (profileName: rec {
    name = "wasmparser";
    version = "0.221.3";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "d06bfa36ab3ac2be0dee563380147a5b81ba10dd8885d7fbbc9eb574be67d185"; };
    features = builtins.concatLists [
      [ "component-model" ]
      [ "simd" ]
      [ "std" ]
    ];
    dependencies = {
      bitflags = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".bitflags."2.8.0" { inherit profileName; }).out;
      indexmap = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".indexmap."2.7.1" { inherit profileName; }).out;
      semver = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".semver."1.0.25" { inherit profileName; }).out;
    };
  });
  
  "unknown".webschembly-compiler."0.1.0" = overridableMkRustCrate (profileName: rec {
    name = "webschembly-compiler";
    version = "0.1.0";
    registry = "unknown";
    src = fetchCrateLocal workspaceSrc;
    dependencies = {
      log = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".log."0.4.26" { inherit profileName; }).out;
      nom = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".nom."7.1.3" { inherit profileName; }).out;
      strum = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".strum."0.27.1" { inherit profileName; }).out;
      strum_macros = (buildRustPackages."registry+https://github.com/rust-lang/crates.io-index".strum_macros."0.27.1" { profileName = "__noProfile"; }).out;
      wasm_encoder = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".wasm-encoder."0.221.3" { inherit profileName; }).out;
    };
  });
  
  "unknown".webschembly-compiler-cli."0.1.0" = overridableMkRustCrate (profileName: rec {
    name = "webschembly-compiler-cli";
    version = "0.1.0";
    registry = "unknown";
    src = fetchCrateLocal workspaceSrc;
    dependencies = {
      anyhow = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".anyhow."1.0.96" { inherit profileName; }).out;
      clap = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".clap."4.5.30" { inherit profileName; }).out;
      webschembly_compiler = (rustPackages."unknown".webschembly-compiler."0.1.0" { inherit profileName; }).out;
    };
  });
  
  "unknown".webschembly-runtime."0.1.0" = overridableMkRustCrate (profileName: rec {
    name = "webschembly-runtime";
    version = "0.1.0";
    registry = "unknown";
    src = fetchCrateLocal workspaceSrc;
    dependencies = {
      log = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".log."0.4.26" { inherit profileName; }).out;
      webschembly_compiler = (rustPackages."unknown".webschembly-compiler."0.1.0" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".windows-sys."0.59.0" = overridableMkRustCrate (profileName: rec {
    name = "windows-sys";
    version = "0.59.0";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "1e38bc4d79ed67fd075bcc251a1c39b32a1776bbe92e5bef1f0bf1f8c531853b"; };
    features = builtins.concatLists [
      [ "Win32" ]
      [ "Win32_Foundation" ]
      [ "Win32_System" ]
      [ "Win32_System_Console" ]
      [ "default" ]
    ];
    dependencies = {
      windows_targets = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".windows-targets."0.52.6" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".windows-targets."0.52.6" = overridableMkRustCrate (profileName: rec {
    name = "windows-targets";
    version = "0.52.6";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "9b724f72796e036ab90c1021d4780d4d3d648aca59e491e6b98e725b84e99973"; };
    dependencies = {
      ${ if hostPlatform.config == "aarch64-pc-windows-gnullvm" then "windows_aarch64_gnullvm" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".windows_aarch64_gnullvm."0.52.6" { inherit profileName; }).out;
      ${ if hostPlatform.parsed.cpu.name == "aarch64" && hostPlatform.parsed.abi.name == "msvc" then "windows_aarch64_msvc" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".windows_aarch64_msvc."0.52.6" { inherit profileName; }).out;
      ${ if hostPlatform.parsed.cpu.name == "i686" && hostPlatform.parsed.abi.name == "gnu" then "windows_i686_gnu" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".windows_i686_gnu."0.52.6" { inherit profileName; }).out;
      ${ if hostPlatform.config == "i686-pc-windows-gnullvm" then "windows_i686_gnullvm" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".windows_i686_gnullvm."0.52.6" { inherit profileName; }).out;
      ${ if hostPlatform.parsed.cpu.name == "i686" && hostPlatform.parsed.abi.name == "msvc" then "windows_i686_msvc" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".windows_i686_msvc."0.52.6" { inherit profileName; }).out;
      ${ if hostPlatform.parsed.cpu.name == "x86_64" && hostPlatform.parsed.abi.name == "gnu" then "windows_x86_64_gnu" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".windows_x86_64_gnu."0.52.6" { inherit profileName; }).out;
      ${ if hostPlatform.config == "x86_64-pc-windows-gnullvm" then "windows_x86_64_gnullvm" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".windows_x86_64_gnullvm."0.52.6" { inherit profileName; }).out;
      ${ if (hostPlatform.parsed.cpu.name == "x86_64" || hostPlatform.parsed.cpu.name == "arm64ec") && hostPlatform.parsed.abi.name == "msvc" then "windows_x86_64_msvc" else null } = (rustPackages."registry+https://github.com/rust-lang/crates.io-index".windows_x86_64_msvc."0.52.6" { inherit profileName; }).out;
    };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".windows_aarch64_gnullvm."0.52.6" = overridableMkRustCrate (profileName: rec {
    name = "windows_aarch64_gnullvm";
    version = "0.52.6";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "32a4622180e7a0ec044bb555404c800bc9fd9ec262ec147edd5989ccd0c02cd3"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".windows_aarch64_msvc."0.52.6" = overridableMkRustCrate (profileName: rec {
    name = "windows_aarch64_msvc";
    version = "0.52.6";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "09ec2a7bb152e2252b53fa7803150007879548bc709c039df7627cabbd05d469"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".windows_i686_gnu."0.52.6" = overridableMkRustCrate (profileName: rec {
    name = "windows_i686_gnu";
    version = "0.52.6";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "8e9b5ad5ab802e97eb8e295ac6720e509ee4c243f69d781394014ebfe8bbfa0b"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".windows_i686_gnullvm."0.52.6" = overridableMkRustCrate (profileName: rec {
    name = "windows_i686_gnullvm";
    version = "0.52.6";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "0eee52d38c090b3caa76c563b86c3a4bd71ef1a819287c19d586d7334ae8ed66"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".windows_i686_msvc."0.52.6" = overridableMkRustCrate (profileName: rec {
    name = "windows_i686_msvc";
    version = "0.52.6";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "240948bc05c5e7c6dabba28bf89d89ffce3e303022809e73deaefe4f6ec56c66"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".windows_x86_64_gnu."0.52.6" = overridableMkRustCrate (profileName: rec {
    name = "windows_x86_64_gnu";
    version = "0.52.6";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "147a5c80aabfbf0c7d901cb5895d1de30ef2907eb21fbbab29ca94c5b08b1a78"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".windows_x86_64_gnullvm."0.52.6" = overridableMkRustCrate (profileName: rec {
    name = "windows_x86_64_gnullvm";
    version = "0.52.6";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "24d5b23dc417412679681396f2b49f3de8c1473deb516bd34410872eff51ed0d"; };
  });
  
  "registry+https://github.com/rust-lang/crates.io-index".windows_x86_64_msvc."0.52.6" = overridableMkRustCrate (profileName: rec {
    name = "windows_x86_64_msvc";
    version = "0.52.6";
    registry = "registry+https://github.com/rust-lang/crates.io-index";
    src = fetchCratesIo { inherit name version; sha256 = "589f6da84c646204747d1270a2a5661ea66ed1cced2631d546fdfb155959f9ec"; };
  });
  
}
