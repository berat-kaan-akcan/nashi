#pragma once

#include <QString>
#include <QStringList>

// Translates the English messages nashi-cli reports (docs/CLI-PROTOCOL.md).
// A message outside the table stays in English.
namespace EngineText {

QString translate(const QString &message);

// The table's source strings, with %1, %2 where the engine fills in a value.
QStringList sources();

} // namespace EngineText
