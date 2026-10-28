#pragma once

#include <QQuickImageProvider>

// image://icon/<freedesktop-icon-name> → themed icon, with Nashi's own icon
// as the fallback.
class IconProvider : public QQuickImageProvider
{
public:
    IconProvider();
    QPixmap requestPixmap(const QString &id, QSize *size, const QSize &requestedSize) override;

    static QIcon appIcon();
};
