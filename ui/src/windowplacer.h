#pragma once

#include <QObject>
#include <QStringList>
#include <QVariantList>
#include <QtQmlIntegration/qqmlintegration.h>

// Moves and resizes another application's window on KDE Plasma by loading a
// short KWin script over D-Bus (org.kde.KWin /Scripting). Wayland gives apps
// no other way to place foreign windows. The target is the topmost normal
// window that is not one of Nashi's own.
class WindowPlacer : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

public:
    explicit WindowPlacer(QObject *parent = nullptr);

    // Layout ids in the order the picker shows them.
    static QStringList layouts();
    // {x, y, width, height} as fractions of the work area; empty for an unknown id.
    static QVariantList fractions(const QString &layout);
    // The KWin script for `layout`; empty for an unknown id.
    static QString scriptFor(const QString &layout);

    Q_INVOKABLE QStringList layoutIds() const { return layouts(); }
    Q_INVOKABLE QVariantList layoutFractions(const QString &layout) const { return fractions(layout); }
    // True when KWin is on the session bus.
    Q_INVOKABLE bool available() const;
    // Runs the layout on the target window. False when KWin is missing or refused.
    Q_INVOKABLE bool place(const QString &layout);

Q_SIGNALS:
    void placed(const QString &layout, bool ok);
};
