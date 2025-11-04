project(
  'routx',
  'cpp',
  license: 'MIT',
  version: '1.0.0',
  meson_version: '>= 1.3.0',
  default_options: ['warning_level=3', 'cpp_std=c++20'],
)

# Get the library filename suffix
if host_machine.system() == 'windows'
  dynamic_lib_suffix = '.dll'
  static_lib_suffix = '.lib'
elif host_machine.system() == 'darwin'
  dynamic_lib_suffix = '.dylib'
  static_lib_suffix = '.a'
elif host_machine.system() == 'linux'
  dynamic_lib_suffix = '.so'
  static_lib_suffix = '.a'
else
  warning('Unknown target system: ', host_machine.system(), ' - assuming dynamic libraries use \'.so\' and static libraries use \'.a\'')
  dynamic_lib_suffix = '.so'
  static_lib_suffix = '.a'
endif

# Get build arguments
py = import('python').find_installation(pure: false)
wrapper = files('cargo_build_wrapper.py')
wrapper_extra_args = []
if get_option('optimization') == 's' or get_option('optimization') == '2' or get_option('optimization') == '3'
  wrapper_extra_args += ['--release']
endif
if meson.has_external_property('cargo_build_command')
  wrapper_extra_args += ['--cmd', meson.get_external_property('cargo_build_command')]
endif
if meson.has_external_property('cargo_build_target')
  wrapper_extra_args += ['--target', meson.get_external_property('cargo_build_target')]
endif

# Build the libraries by executing cargo
routx = custom_target(
  'routx',
  output: ['libroutx' + dynamic_lib_suffix, 'libroutx' + static_lib_suffix],
  command: [
    py,
    wrapper,
    wrapper_extra_args,
    '--out-dynamic', '@OUTPUT0@',
    '--out-static', '@OUTPUT1@',
    '@CURRENT_SOURCE_DIR@',
  ],
  console: true,
  build_always_stale: true,
  build_by_default: true,
  install: true,
  install_dir: [get_option('libdir'), false],
)
routx_dynamic = routx[0]
routx_static = routx[1]

# Make this project usable as a Meson subproject
routx_dep = declare_dependency(
  link_with: routx_static,
  include_directories: include_directories('bindings/include'),
  dependencies: [],
)
meson.override_dependency('routx', routx_dep)

# Make routx usable from system's package manager
install_headers('bindings/include/routx.h', 'bindings/include/routx.hpp')
import('pkgconfig').generate(
  name: 'routx',
  description: 'Simple routing over OSM data (C & C++ bindings)',
  libraries: [routx_dynamic],
)

# Get gtest
gtest_main_dep = dependency('gtest_main', fallback: ['gtest', 'gtest_main_dep'])

# Build & run tests for the bindings
test_executable = executable(
  'test-exe',
  ['bindings/tests.cpp'],
  dependencies: [routx_dep, gtest_main_dep],
)
test('test', test_executable)
