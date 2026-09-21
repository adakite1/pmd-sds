use cfg_aliases::cfg_aliases;

fn main() {
    // Check all cfgs with:
    //
    // $ cargo rustc -- -Z unstable-options --print cfg
    //
    // ...inside arm7-bin and arm9-bin folders.
    cfg_aliases! {
        armv4t: { all(target_arch = "arm", target_os = "none", target_abi = "eabi", not(target_feature = "v5te")) },
        armv5te: { all(target_arch = "arm", target_os = "none", target_abi = "eabi", target_feature = "v5te") },
    }
}

