#include "tray.h"
#include "app.h"
#include "iconprovider.h"
#include "inputdclient.h"
#include "language.h"

#include <QAction>
#include <QCoreApplication>
#include <QMenu>
#include <QSystemTrayIcon>

Tray::Tray(App *app, QObject *parent)
    : QObject(parent)
    , m_app(app)
{
    m_menu = new QMenu;
    m_open = m_menu->addAction(QString(), m_app, &App::showHome);
    m_chords = m_menu->addAction(QString());
    m_chords->setCheckable(true);
    connect(m_chords, &QAction::triggered, this, [this](bool on) { m_app->inputd()->setEnabled(on); });
    m_menu->addSeparator();
    m_quit = m_menu->addAction(QString(), m_app, &App::quit);

    m_icon = new QSystemTrayIcon(IconProvider::appIcon(), this);
    m_icon->setContextMenu(m_menu);
    connect(m_icon, &QSystemTrayIcon::activated, this, [this](QSystemTrayIcon::ActivationReason r) {
        if (r == QSystemTrayIcon::Trigger || r == QSystemTrayIcon::DoubleClick)
            m_app->showHome();
    });
    connect(m_app->inputd(), &InputdClient::availableChanged, this, &Tray::syncChords);
    connect(m_app->inputd(), &InputdClient::enabledChanged, this, &Tray::syncChords);
    connect(m_app->language(), &Language::codeChanged, this, &Tray::retranslate);
    retranslate();
    m_icon->show();
}

Tray::~Tray()
{
    delete m_menu;
}

void Tray::retranslate()
{
    m_open->setText(tr("Open Nashi"));
    m_chords->setText(tr("Enable drag chords"));
    m_quit->setText(tr("Quit"));
    m_icon->setToolTip(tr("Nashi — drag a file and hold Shift, Ctrl or Alt"));
    syncChords();
}

void Tray::syncChords()
{
    InputdClient *in = m_app->inputd();
    m_chords->setEnabled(in->available());
    m_chords->setChecked(in->available() && in->enabled());
    if (!in->available())
        m_chords->setText(tr("Enable drag chords (nashi-inputd not running)"));
    else
        m_chords->setText(tr("Enable drag chords"));
}
