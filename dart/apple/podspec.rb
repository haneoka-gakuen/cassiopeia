def configure_cassiopeia_runtime(s)
  s.name = 'cassiopeia_runtime'
  s.version = '0.1.0'
  s.summary = 'Cassiopeia Rust execution core through the platform C ABI.'
  s.description = s.summary
  s.homepage = 'https://github.com/haneoka-gakuen/cassiopeia'
  s.license = { :type => 'MPL-2.0', :file => '../../LICENSE' }
  s.author = 'Cassiopeia contributors'
  s.source = { :path => '.' }
  s.source_files = 'Classes/*.c'
  s.static_framework = true
  symbols = %w[cassiopeia_abi_version cassiopeia_fixed_abi_version cassiopeia_create cassiopeia_destroy cassiopeia_dispatch cassiopeia_reply cassiopeia_update cassiopeia_reset cassiopeia_reset_observed cassiopeia_project_watch_observed cassiopeia_projection_snapshot_words cassiopeia_projection_event_words cassiopeia_finish cassiopeia_snapshot_words cassiopeia_snapshot_word_count cassiopeia_event_words cassiopeia_event_stride cassiopeia_error cassiopeia_error_length]
  retain = symbols.map { |name| "-Wl,-u,_#{name}" }.join(' ')
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES', 'ENABLE_USER_SCRIPT_SANDBOXING' => 'NO' }
  s.user_target_xcconfig = {
    'OTHER_LDFLAGS' => "$(inherited) #{retain} \"$(BUILT_PRODUCTS_DIR)/cassiopeia_runtime/libcassiopeia_native.a\""
  }
  s.script_phase = {
    :name => 'Build Cassiopeia Rust execution core',
    :script => '/bin/sh "${PODS_TARGET_SRCROOT}/../apple/build.sh"',
    :execution_position => :before_compile,
    :always_out_of_date => '1',
    :input_files => [File.realpath('build.sh', __dir__)] +
      Dir[File.expand_path('../../crates/{cassiopeia-core,cassiopeia-native}/**/*.{rs,toml}', __dir__)] +
      %w[Cargo.toml Cargo.lock].map { |name| File.expand_path("../../#{name}", __dir__) },
    :output_files => ['$(BUILT_PRODUCTS_DIR)/cassiopeia_runtime/libcassiopeia_native.a']
  }
end
