#pragma once

#include <QObject>
#include <QTranslator>
#include <QtQmlIntegration/qqmlintegration.h>

class QQmlEngine;

// Installs the UI translation for "auto" (system locale), "en" or "tr" and
// tells QML engines to re-evaluate qsTr() bindings.
class Language : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(QString code READ code NOTIFY codeChanged) // effective: "en" | "tr"

public:
    explicit Language(QObject *parent = nullptr);

    static QString resolve(const QString &setting, const QLocale &system);

    QString code() const { return m_code; }
    void apply(const QString &setting);
    void addEngine(QQmlEngine *engine);

Q_SIGNALS:
    void codeChanged();

private:
    QString m_code = QStringLiteral("en");
    QTranslator m_app;
    QTranslator m_qt;
    bool m_installed = false;
    QList<QQmlEngine *> m_engines;
};
