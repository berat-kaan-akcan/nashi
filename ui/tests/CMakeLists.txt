# Qt Test suites; all run headless (QT_QPA_PLATFORM=offscreen) against the
# fake CLI fixture, which is copied with execute permission.
find_package(Python3 COMPONENTS Interpreter)

# configure_file re-copies whenever the fixture changes.
configure_file(fake-nashi-cli ${CMAKE_CURRENT_BINARY_DIR}/fake-nashi-cli COPYONLY
               FILE_PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE GROUP_READ GROUP_EXECUTE WORLD_READ WORLD_EXECUTE)
set(FAKE_CLI ${CMAKE_CURRENT_BINARY_DIR}/fake-nashi-cli)

function(nashi_add_test name)
    qt_add_executable(${name} ${name}.cpp)
    target_link_libraries(${name} PRIVATE nashi_ui nashi_uiplugin Qt6::Test)
    target_compile_definitions(${name} PRIVATE FAKE_CLI="${FAKE_CLI}"
                               NASHI_QML_SRC_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../qml"
                               NASHI_I18N_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../i18n")
    add_test(NAME ${name} COMMAND ${name})
    set(env
        "QT_QPA_PLATFORM=offscreen"
        "NASHI_CLI=${FAKE_CLI}"
        "XDG_CONFIG_HOME=${CMAKE_CURRENT_BINARY_DIR}/xdg-config-${name}"
        "XDG_CACHE_HOME=${CMAKE_CURRENT_BINARY_DIR}/xdg-cache-${name}"
        "XDG_DATA_HOME=${CMAKE_CURRENT_BINARY_DIR}/xdg-data-${name}"
        "DBUS_SESSION_BUS_ADDRESS=unix:path=${CMAKE_CURRENT_BINARY_DIR}/no-such-bus"
        "LANG=C.UTF-8"
        "LANGUAGE=")
    set_tests_properties(${name} PROPERTIES ENVIRONMENT "${env}" TIMEOUT 60)
    if(NOT Python3_Interpreter_FOUND AND NOT ${name} STREQUAL "tst_wheelgeometry")
        set_tests_properties(${name} PROPERTIES DISABLED TRUE)
    endif()
endfunction()

nashi_add_test(tst_jsonlines)
nashi_add_test(tst_jobrunner)
nashi_add_test(tst_clilocator)
nashi_add_test(tst_wheelmodel)
nashi_add_test(tst_wheelgeometry)
nashi_add_test(tst_cliargs)
nashi_add_test(tst_overlay)
nashi_add_test(tst_misc)
nashi_add_test(tst_windowplacer)
nashi_add_test(tst_clipboardimport)
nashi_add_test(tst_qmlsmoke)
nashi_add_test(tst_userconfig)
nashi_add_test(tst_setupcheck)
nashi_add_test(tst_editors)
nashi_add_test(tst_desktop)

# The Turkish translation must stay complete.
foreach(lang tr fr)
    add_test(NAME tst_translations_complete_${lang}
             COMMAND ${CMAKE_COMMAND} -DTS_FILE=${CMAKE_CURRENT_SOURCE_DIR}/../i18n/nashi_${lang}.ts
                     -P ${CMAKE_CURRENT_SOURCE_DIR}/check_translations.cmake)
endforeach()

# Forms against the real engine; skipped until `cargo build` has made nashi-cli.
set(NASHI_REAL_CLI "${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug/nashi-cli" CACHE FILEPATH "nashi-cli for tst_forms_engine")
nashi_add_test(tst_forms_engine)
set_tests_properties(tst_forms_engine PROPERTIES TIMEOUT 300)
set_property(TEST tst_forms_engine APPEND PROPERTY ENVIRONMENT "NASHI_CLI=${NASHI_REAL_CLI}")
