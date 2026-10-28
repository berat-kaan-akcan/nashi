#include "clilocator.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QStandardPaths>

namespace {

bool isExecutableFile(const QString &path)
{
    const QFileInfo fi(path);
    return fi.isFile() && fi.isExecutable();
}

} // namespace

namespace CliLocator {

QString locate(const QString &envValue, const QString &appDir, const QString &pathEnv)
{
    const QString name = QStringLiteral("nashi-cli");
    if (!envValue.isEmpty()) {
        if (isExecutableFile(envValue))
            return QFileInfo(envValue).absoluteFilePath();
        // A bare program name in $NASHI_CLI is looked up in PATH.
        if (!envValue.contains(QLatin1Char('/'))) {
            const QString found = QStandardPaths::findExecutable(
                envValue, pathEnv.split(QLatin1Char(':'), Qt::SkipEmptyParts));
            if (!found.isEmpty())
                return found;
        }
    }
    if (!appDir.isEmpty()) {
        const QString sibling = QDir(appDir).filePath(name);
        if (isExecutableFile(sibling))
            return sibling;
    }
    return QStandardPaths::findExecutable(name, pathEnv.split(QLatin1Char(':'), Qt::SkipEmptyParts));
}

QString locate()
{
    return locate(qEnvironmentVariable("NASHI_CLI"), QCoreApplication::applicationDirPath(),
                  qEnvironmentVariable("PATH"));
}

} // namespace CliLocator
