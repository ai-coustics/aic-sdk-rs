use std::path::Path;
use std::process::Command;

pub fn patch_lib(
    static_lib: &Path,
    out_dir: &Path,
    lib_name: &str,
    lib_name_patched: &str,
    global_symbols_wildcard: &str,
    final_lib: &Path,
) {
    // Original .o file
    let intermediate_obj = out_dir.join(format!("lib{}.o", lib_name));

    // Modified .o file
    let final_obj = out_dir.join(format!("lib{}.o", lib_name_patched));

    // Partially link
    let ld_status = Command::new("ld")
        .arg("-r")
        .arg("-o")
        .arg(&intermediate_obj)
        .arg("--whole-archive")
        .arg(static_lib)
        .status()
        .expect("Failed to execute ld command.");

    if !ld_status.success() {
        panic!("ld -r command failed for {}", static_lib.display());
    }

    // Curate symbols (only keep specific symbols) and remove problematic sections
    let objcopy_status = Command::new("objcopy")
        .arg("--wildcard")
        .arg("--keep-global-symbol")
        .arg(global_symbols_wildcard)
        // Remove sections that may contain references to stripped Rust compiler symbols
        .arg("--remove-section=.eh_frame")
        .arg("--remove-section=.eh_frame_hdr")
        .arg("--remove-section=.gcc_except_table")
        .arg("--remove-section=.debug_*")
        .arg("--remove-section=.note.gnu.build-id")
        .arg("--remove-section=.comment")
        .arg(&intermediate_obj)
        .arg(&final_obj)
        .status()
        .expect("Failed to execute objcopy command.");

    if !objcopy_status.success() {
        panic!("objcopy command failed for {}", intermediate_obj.display());
    }

    // Build the archive
    let ar_status = Command::new("ar")
        .arg("rcs")
        .arg(final_lib)
        .arg(&final_obj)
        .status()
        .expect("Failed to execute ar.");

    if !ar_status.success() {
        panic!("ar command failed for {}", final_obj.display());
    }
}
