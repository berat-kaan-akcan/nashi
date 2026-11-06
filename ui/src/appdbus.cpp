#include "appdbus.h"
#include "app.h"

#include <QUrl>

namespace {
void applyActivationToken(const QVariantMap &platformData)
{
    const QString token = platformData.value(QStringLiteral("activation-token")).toString();
    if (!token.isEmpty())
        qputenv("XDG_ACTIVATION_TOKEN", token.toUtf8());
}
} // namespace

NashiAdaptor::NashiAdaptor(App *app)
    : QDBusAbstractAdaptor(app)
    , m_app(app)
{
}

void NashiAdaptor::ShowWheel(const QStringList &paths, const QString &mode)
{
    m_app->showWheel(paths, mode);
}

void NashiAdaptor::ShowWheelFromClipboard(const QString &mode)
{
    m_app->showWheelFromClipboard(mode);
}

void NashiAdaptor::Activate()
{
    m_app->showHome();
}

void NashiAdaptor::Download(const QString &url)
{
    if (m_app->isValidWebUrl(url))
        m_app->showDownload(url.trimmed(), false);
}

void NashiAdaptor::DownloadClipboard()
{
    m_app->showDownloadFromClipboard();
}

void NashiAdaptor::ToggleShelf()
{
    m_app->toggleShelf();
}

void NashiAdaptor::AddToShelf(const QStringList &paths)
{
    m_app->addToShelf(paths);
}

void NashiAdaptor::ShowWindowPlacer()
{
    m_app->showWindowPlacer();
}

FdoApplicationAdaptor::FdoApplicationAdaptor(App *app)
    : QDBusAbstractAdaptor(app)
    , m_app(app)
{
}

void FdoApplicationAdaptor::Activate(const QVariantMap &platformData)
{
    applyActivationToken(platformData);
    m_app->showHome();
}

void FdoApplicationAdaptor::Open(const QStringList &uris, const QVariantMap &platformData)
{
    applyActivationToken(platformData);
    QStringList files;
    for (const QString &u : uris) {
        const QUrl url(u);
        if (url.isLocalFile())
            files.append(url.toLocalFile());
    }
    m_app->openFiles(files);
}

void FdoApplicationAdaptor::ActivateAction(const QString &name, const QVariantList &, const QVariantMap &platformData)
{
    applyActivationToken(platformData);
    if (name == QLatin1String("quit"))
        m_app->quit();
    else if (name == QLatin1String("DownloadClipboard"))
        m_app->showDownloadFromClipboard();
    else if (name == QLatin1String("PlaceWindow"))
        m_app->showWindowPlacer();
    else
        m_app->showHome();
}
