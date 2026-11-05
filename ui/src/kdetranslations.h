#pragma once

#include <QObject>
#include <QVariant>

// The i18n functions KDE's QML expects from its engine. KDE Purpose's share
// plugins call them, and their text stays empty without them. Reads KDE's own
// catalogs with gettext, so this adds no KDE library to the build.
class KdeTranslations : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    Q_INVOKABLE QString i18n(const QString &text, const QVariant &a1 = {}, const QVariant &a2 = {}) const;
    Q_INVOKABLE QString i18nd(const QString &domain, const QString &text, const QVariant &a1 = {},
                              const QVariant &a2 = {}) const;
};
