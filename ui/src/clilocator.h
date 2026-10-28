#pragma once

#include <QString>

namespace CliLocator {

// Resolution order (docs/ARCHITECTURE.md): $NASHI_CLI, `nashi-cli` next to
// the running executable, then PATH. Returns an empty string when not found.
QString locate();

// Pure variant used by locate() and the tests.
QString locate(const QString &envValue, const QString &appDir, const QString &pathEnv);

} // namespace CliLocator
