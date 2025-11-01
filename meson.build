project(
  'routex',
  'cpp',
  license: 'MIT',
  version: '0.1.0',
  meson_version : '>= 1.3.0',
  default_options : ['warning_level=3', 'cpp_std=c++20'],
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
if meson.is_cross_build()
  wrapper_extra_args += ['--target', meson.get_external_property('cargo_target')]
endif

# Build the libraries by executing cargo
routex = custom_target(
  'routex',
  output: ['libroutex' + dynamic_lib_suffix, 'libroutex' + static_lib_suffix],
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
routex_dynamic = routex[0]
routex_static = routex[1]

# Make this project usable as a Meson subproject
routex_dep = declare_dependency(
  link_with: routex_static,
  include_directories: include_directories('bindings/include'),
  dependencies: [],
)
meson.override_dependency('routex', routex_dep)

# Make routex usable from system's package manager
install_headers('bindings/include/routex.h', 'bindings/include/routex.hpp')
import('pkgconfig').generate(
  name: 'routex',
  description: 'Simple routing over OSM data (C & C++ bindings)',
  libraries: [routex_dynamic],
)

# Get gtest
gtest_main_dep = dependency('gtest_main', fallback: ['gtest', 'gtest_main_dep'])

# Build & run tests for the bindings
test_executable = executable(
  'test-exe',
  ['bindings/tests.cpp'],
  dependencies: [routex_dep, gtest_main_dep],
)
test('test', test_executable)
