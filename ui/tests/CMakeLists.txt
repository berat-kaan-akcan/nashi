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
    target_compile_definitions(${name} PRIVATE FAKE_CLI="${FAKE_CLI}")
    add_test(NAME ${name} COMMAND ${name})
    set(env
        "QT_QPA_PLATFORM=offscreen"
        "NASHI_CLI=${FAKE_CLI}"
        "XDG_CONFIG_HOME=${CMAKE_CURRENT_BINARY_DIR}/xdg-config-${name}"
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
nashi_add_test(tst_qmlsmoke)

# The Turkish translation must stay complete.
add_test(NAME tst_translations_complete
         COMMAND ${CMAKE_COMMAND} -DTS_FILE=${CMAKE_CURRENT_SOURCE_DIR}/../i18n/nashi_tr.ts
                 -P ${CMAKE_CURRENT_SOURCE_DIR}/check_translations.cmake)
