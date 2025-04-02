use std::ffi::c_char;

//--------------------------------------------------------------------------------------------------
// FFI
//--------------------------------------------------------------------------------------------------

#[link(name = "krun")]
extern "C" {
    /// Sets the log level for the library.
    ///
    /// ## Arguments
    ///
    /// * `level` - The log level to set. The values for the different levels are:
    ///   - `0` - Off
    ///   - `1` - Error
    ///   - `2` - Warn
    ///   - `3` - Info
    ///   - `4` - Debug
    ///   - `5` - Trace
    pub(crate) fn krun_set_log_level(level: u32) -> i32;

    /// Creates a configuration context.
    ///
    /// ## Returns
    ///
    /// Returns the context ID on success or a negative error number on failure.
    pub(crate) fn krun_create_ctx() -> i32;

    /// Frees an existing configuration context.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID to free.
    pub(crate) fn krun_free_ctx(ctx_id: u32) -> i32;

    /// Sets the basic configuration parameters for the MicroVm.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `num_vcpus` - The number of vCPUs.
    /// * `ram_mib` - The amount of RAM in MiB.
    pub(crate) fn krun_set_vm_config(ctx_id: u32, num_vcpus: u8, ram_mib: u32) -> i32;

    /// Sets the path to be used as root for the MicroVm.
    ///
    /// Not available in libkrun-SEV.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `root_path` - The path to be used as root.
    ///
    /// ## Returns
    ///
    /// Returns 0 on success or a negative error code on failure.
    ///
    /// ## Errors
    ///
    /// * `-EEXIST` - A root device is already set
    ///
    /// ## Notes
    ///
    /// This function is mutually exclusive with `krun_set_overlayfs_root`.
    pub(crate) fn krun_set_root(ctx_id: u32, root_path: *const c_char) -> i32;

    /// Sets up an OverlayFS to be used as root for the MicroVm.
    ///
    /// Not available in libkrun-SEV.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `root_layers` - A null-terminated array of string pointers representing filesystem paths
    ///   to be used as layers for the OverlayFS. Must contain at least one layer.
    ///
    /// ## Returns
    ///
    /// Returns 0 on success or a negative error code on failure.
    ///
    /// ## Errors
    ///
    /// * `-EINVAL` - No layers are provided
    /// * `-EEXIST` - A root device is already set
    ///
    /// ## Notes
    ///
    /// This function is mutually exclusive with `krun_set_root`.
    pub(crate) fn krun_set_overlayfs_root(ctx_id: u32, root_layers: *const *const c_char) -> i32;

    /// Adds a disk image to be used as a general partition for the MicroVm.
    ///
    /// This API is mutually exclusive with the deprecated krun_set_root_disk and
    /// krun_set_data_disk methods and must not be used together.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `block_id` - A null-terminated string representing the partition.
    /// * `disk_path` - A null-terminated string representing the path leading to the disk image that
    ///   contains the root file-system.
    /// * `read_only` - Whether the mount should be read-only. Required if the caller does not have
    ///   write permissions (for disk images in /usr/share).
    #[allow(dead_code)]
    pub(crate) fn krun_add_disk(
        ctx_id: u32,
        block_id: *const c_char,
        disk_path: *const c_char,
        read_only: bool,
    ) -> i32;

    /// Adds an independent virtio-fs device pointing to a host's directory with a tag.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_tag` - The tag to identify the filesystem in the guest.
    /// * `c_path` - The full path to the host's directory to be exposed to the guest.
    pub(crate) fn krun_add_virtiofs(
        ctx_id: u32,
        c_tag: *const c_char,
        c_path: *const c_char,
    ) -> i32;

    /// Adds an independent virtio-fs device pointing to a host's directory with a tag. This variant
    /// allows specifying the size of the DAX window.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_tag` - The tag to identify the filesystem in the guest.
    /// * `c_path` - The full path to the directory in the host to be exposed to the guest.
    /// * `shm_size` - The size of the DAX SHM window in bytes.
    #[allow(dead_code)]
    pub(crate) fn krun_add_virtiofs2(
        ctx_id: u32,
        c_tag: *const c_char,
        c_path: *const c_char,
        shm_size: u64,
    ) -> i32;

    /// Configures the networking to use passt.
    /// Calling this function disables TSI backend to use passt instead.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `fd` - A file descriptor to communicate with passt.
    #[allow(dead_code)]
    pub(crate) fn krun_set_passt_fd(ctx_id: u32, fd: i32) -> i32;

    /// Configures the networking to use gvproxy in vfkit mode.
    /// Calling this function disables TSI backend to use gvproxy instead.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_path` - The path to the gvproxy binary.
    ///
    /// ## Note
    ///
    /// If you never call this function, networking uses the TSI backend.
    /// This function should be called before krun_set_port_map.
    #[allow(dead_code)]
    pub(crate) fn krun_set_gvproxy_path(ctx_id: u32, c_path: *const c_char) -> i32;

    /// Sets the MAC address for the virtio-net device when using the passt backend.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_mac` - The MAC address as an array of 6 uint8_t entries.
    #[allow(dead_code)]
    pub(crate) fn krun_set_net_mac(ctx_id: u32, c_mac: *const u8) -> i32;

    /// Configures a map of host to guest TCP ports for the MicroVm.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_port_map` - A **null-terminated** array of string pointers with format
    ///   "host_port:guest_port".
    ///
    /// ## Note
    ///
    /// Passing NULL (or not calling this function) as "port_map" has a different meaning than
    /// passing an empty array. The first one will instruct libkrun to attempt to expose all
    /// listening ports in the guest to the host, while the second means that no port from the
    /// guest will be exposed to host.
    ///
    /// Exposed ports will only become accessible by their "host_port" in the guest too. This
    /// means that for a map such as "8080:80", applications running inside the guest will also
    /// need to access the service through the "8080" port.
    ///
    /// If passt networking mode is used (krun_set_passt_fd was called), port mapping is not
    /// supported as an API of libkrun (but you can still do port mapping using command line
    /// arguments of passt).
    pub(crate) fn krun_set_port_map(ctx_id: u32, c_port_map: *const *const c_char) -> i32;

    /// Configures the static IP, subnet, and scope for the TSI network backend.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_ip` - An optional null-terminated string representing the guest's static IPv4 address.
    /// * `c_subnet` - An optional null-terminated string representing the guest's subnet in CIDR notation (e.g., "192.168.1.0/24").
    /// * `scope` - An integer specifying the scope (0-3):
    ///   - `0` - None - Block all IP communication
    ///   - `1` - Group - Allow within subnet (if specified; otherwise, block all like scope 0)
    ///   - `2` - Public - Allow public IPs
    ///   - `3` - Any - Allow any IP
    ///
    /// ## Returns
    ///
    /// Returns 0 on success or a negative error number on failure.
    ///
    /// ## Errors
    ///
    /// * `-EINVAL` - If scope value is > 3 or IP/subnet strings are invalid.
    /// * `-ENOTSUP` - If the network mode is not TSI.
    ///
    /// ## Notes
    ///
    /// This function is only effective when the default TSI network backend is used (i.e., neither
    /// `krun_set_passt_fd` nor `krun_set_gvproxy_path` has been called).
    pub(crate) fn krun_set_tsi_scope(
        ctx_id: u32,
        c_ip: *const c_char,
        c_subnet: *const c_char,
        scope: u8,
    ) -> i32;

    /// Enables and configures a virtio-gpu device.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `virgl_flags` - Flags to pass to virglrenderer.
    #[allow(dead_code)]
    pub(crate) fn krun_set_gpu_options(ctx_id: u32, virgl_flags: u32) -> i32;

    /// Enables and configures a virtio-gpu device. This variant allows specifying the size of the
    /// host window (acting as vRAM in the guest).
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `virgl_flags` - Flags to pass to virglrenderer.
    /// * `shm_size` - The size of the SHM host window in bytes.
    #[allow(dead_code)]
    pub(crate) fn krun_set_gpu_options2(ctx_id: u32, virgl_flags: u32, shm_size: u64) -> i32;

    /// Enables or disables a virtio-snd device.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `enable` - Whether to enable the sound device.
    #[allow(dead_code)]
    pub(crate) fn krun_set_snd_device(ctx_id: u32, enable: bool) -> i32;

    /// Configures a map of rlimits to be set in the guest before starting the isolated binary.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_rlimits` - A **null-terminated** array of string pointers with format
    ///   "<RESOURCE_NUMBER>=RLIM_CUR:RLIM_MAX" (e.g., "6=1024:1024").
    pub(crate) fn krun_set_rlimits(ctx_id: u32, c_rlimits: *const *const c_char) -> i32;

    /// Sets the SMBIOS OEM strings for the MicroVm.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_oem_strings` - An array of string pointers. Must be terminated with an additional NULL
    ///   pointer.
    #[allow(dead_code)]
    pub(crate) fn krun_set_smbios_oem_strings(
        ctx_id: u32,
        c_oem_strings: *const *const c_char,
    ) -> i32;

    /// Sets the working directory for the executable to be run inside the MicroVm.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_workdir_path` - The path to the working directory, relative to the root configured with
    ///   "krun_set_root".
    pub(crate) fn krun_set_workdir(ctx_id: u32, c_workdir_path: *const c_char) -> i32;

    /// Sets the path to the executable to be run inside the MicroVm, the arguments to be passed to
    /// the executable, and the environment variables to be configured in the context of the
    /// executable.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_exec_path` - The path to the executable, relative to the root configured with
    ///   "krun_set_root".
    /// * `c_argv` - A **null-terminated** array of string pointers to be passed as arguments.
    /// * `c_envp` - A **null-terminated** array of string pointers to be injected as environment
    ///   variables into the context of the executable.
    ///
    /// ## Note
    ///
    /// Passing NULL for `c_envp` will auto-generate an array collecting the the variables currently
    /// present in the environment.
    pub(crate) fn krun_set_exec(
        ctx_id: u32,
        c_exec_path: *const c_char,
        c_argv: *const *const c_char,
        c_envp: *const *const c_char,
    ) -> i32;

    /// Sets the environment variables to be configured in the context of the executable.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_envp` - A **null-terminated** array of string pointers to be injected as environment
    ///   variables into the context of the executable.
    ///
    /// ## Note
    ///
    /// Passing NULL for `c_envp` will auto-generate an array collecting the the variables currently
    /// present in the environment.
    #[allow(dead_code)]
    pub(crate) fn krun_set_env(ctx_id: u32, c_envp: *const *const c_char) -> i32;

    /// Sets the filepath to the TEE configuration file for the MicroVm. Only available in
    /// libkrun-sev.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_filepath` - The filepath to the TEE configuration file.
    #[allow(dead_code)]
    pub(crate) fn krun_set_tee_config_file(ctx_id: u32, c_filepath: *const c_char) -> i32;

    /// Adds a port-path pairing for guest IPC with a process in the host.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `port` - The port that the guest will connect to for IPC.
    /// * `c_filepath` - The path of the UNIX socket in the host.
    #[allow(dead_code)]
    pub(crate) fn krun_add_vsock_port(ctx_id: u32, port: u32, c_filepath: *const c_char) -> i32;

    /// Gets the eventfd file descriptor to signal the guest to shut down orderly. This must be
    /// called before starting the MicroVm with "krun_start_enter". Only available in libkrun-efi.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    ///
    /// ## Returns
    ///
    /// Returns the eventfd file descriptor on success or a negative error number on failure.
    #[allow(dead_code)]
    pub(crate) fn krun_get_shutdown_eventfd(ctx_id: u32) -> i32;

    /// Sets the path to the file to write the console output for the MicroVm.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    /// * `c_filepath` - The path of the file to write the console output.
    pub(crate) fn krun_set_console_output(ctx_id: u32, c_filepath: *const c_char) -> i32;

    /// Starts and enters the MicroVm with the configured parameters. The VMM will attempt to take over
    /// stdin/stdout to manage them on behalf of the process running inside the isolated environment,
    /// simulating that the latter has direct control of the terminal.
    ///
    /// This function consumes the configuration pointed by the context ID.
    ///
    /// ## Arguments
    ///
    /// * `ctx_id` - The configuration context ID.
    ///
    /// ## Returns
    ///
    /// This function only returns if an error happens before starting the MicroVm. Otherwise, the
    /// VMM assumes it has full control of the process, and will call to exit() once the MicroVm shuts
    /// down.
    pub(crate) fn krun_start_enter(ctx_id: u32) -> i32;
}
