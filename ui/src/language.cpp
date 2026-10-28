#include "language.h"

#include <QCoreApplication>
#include <QLibraryInfo>
#include <QLocale>
#include <QQmlEngine>

Language::Language(QObject *parent)
    : QObject(parent)
{
}

QString Language::resolve(const QString &setting, const QLocale &system)
{
    if (setting == QLatin1String("en") || setting == QLatin1String("tr"))
        return setting;
    return system.language() == QLocale::Turkish ? QStringLiteral("tr") : QStringLiteral("en");
}

void Language::apply(const QString &setting)
{
    const QString code = resolve(setting, QLocale::system());
    if (m_installed) {
        QCoreApplication::removeTranslator(&m_app);
        QCoreApplication::removeTranslator(&m_qt);
        m_installed = false;
    }
    if (code != QLatin1String("en")) {
        // qt_add_translations() embeds the .qm files under :/i18n.
        if (m_app.load(QStringLiteral(":/i18n/nashi_%1.qm").arg(code)))
            QCoreApplication::installTranslator(&m_app);
        if (m_qt.load(QStringLiteral("qt_%1").arg(code), QLibraryInfo::path(QLibraryInfo::TranslationsPath)))
            QCoreApplication::installTranslator(&m_qt);
        m_installed = true;
    }
    QLocale::setDefault(QLocale(code == QLatin1String("tr") ? QLocale::Turkish : QLocale::English));
    const bool changed = code != m_code;
    m_code = code;
    for (QQmlEngine *e : std::as_const(m_engines))
        e->retranslate();
    if (changed)
        Q_EMIT codeChanged();
}

void Language::addEngine(QQmlEngine *engine)
{
    m_engines.append(engine);
    connect(engine, &QObject::destroyed, this, [this, engine] { m_engines.removeAll(engine); });
}
