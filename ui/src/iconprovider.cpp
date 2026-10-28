#include "iconprovider.h"

#include <QIcon>

IconProvider::IconProvider()
    : QQuickImageProvider(QQuickImageProvider::Pixmap)
{
}

QIcon IconProvider::appIcon()
{
    static const QIcon fallback(QStringLiteral(":/qt/qml/NashiApp/resources/nashi.svg"));
    return QIcon::fromTheme(QStringLiteral("io.github.beratkaanakcan.Nashi"), fallback);
}

QPixmap IconProvider::requestPixmap(const QString &id, QSize *size, const QSize &requestedSize)
{
    const QSize s = requestedSize.isValid() && !requestedSize.isEmpty() ? requestedSize : QSize(32, 32);
    QIcon icon = id.isEmpty() ? QIcon() : QIcon::fromTheme(id);
    if (icon.isNull())
        icon = appIcon();
    const QPixmap pm = icon.pixmap(s);
    if (size)
        *size = pm.size();
    return pm;
}
