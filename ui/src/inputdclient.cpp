#include "inputdclient.h"

#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusServiceWatcher>
#include <QDBusVariant>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QLoggingCategory>
#include <QRegularExpression>
#include <QSaveFile>

Q_LOGGING_CATEGORY(lcInputd, "nashi.inputd")

namespace {
const QString kProps = QStringLiteral("org.freedesktop.DBus.Properties");

// Strips a `# comment` that is not inside a string.
QString stripComment(const QString &line)
{
    QChar quote;
    for (int i = 0; i < line.size(); ++i) {
        const QChar c = line.at(i);
        if (!quote.isNull()) {
            if (c == QLatin1Char('\\') && quote == QLatin1Char('"'))
                ++i;
            else if (c == quote)
                quote = QChar();
        } else if (c == QLatin1Char('"') || c == QLatin1Char('\'')) {
            quote = c;
        } else if (c == QLatin1Char('#')) {
            return line.left(i);
        }
    }
    return line;
}

// `[name]` header → name (quotes and spaces removed); empty when the line is
// not a table header. Array-of-tables headers (`[[x]]`) return "[[".
QString sectionOf(const QString &code)
{
    const QString t = code.trimmed();
    if (t.startsWith(QLatin1String("[[")))
        return QStringLiteral("[[");
    if (!t.startsWith(QLatin1Char('[')) || !t.endsWith(QLatin1Char(']')))
        return {};
    QString name = t.mid(1, t.size() - 2);
    name.remove(QLatin1Char('"')).remove(QLatin1Char('\'')).remove(QLatin1Char(' ')).remove(QLatin1Char('\t'));
    return name;
}

// `key = value` → (key without quotes/spaces, raw value); key empty when the
// line is not a key/value pair.
std::pair<QString, QString> keyValueOf(const QString &code)
{
    const int eq = code.indexOf(QLatin1Char('='));
    if (eq <= 0)
        return {};
    QString key = code.left(eq);
    key.remove(QLatin1Char('"')).remove(QLatin1Char('\'')).remove(QLatin1Char(' ')).remove(QLatin1Char('\t'));
    return {key, code.mid(eq + 1).trimmed()};
}

QString unquote(const QString &v)
{
    const QString t = v.trimmed();
    if (t.size() >= 2 && (t.front() == QLatin1Char('"') || t.front() == QLatin1Char('\'')) && t.back() == t.front())
        return t.mid(1, t.size() - 2);
    return t;
}

QString normaliseMode(const QString &v)
{
    return InputdConfig::modes().contains(v) ? v : QString();
}
} // namespace

QStringList InputdConfig::modifierKeys()
{
    return {QStringLiteral("shift"), QStringLiteral("ctrl"), QStringLiteral("alt"), QStringLiteral("meta")};
}

QStringList InputdConfig::modes()
{
    return {QStringLiteral("convert"), QStringLiteral("tools"), QStringLiteral("actions")};
}

QString InputdConfig::defaultPath()
{
    QString base = qEnvironmentVariable("XDG_CONFIG_HOME");
    if (base.isEmpty() || !QDir::isAbsolutePath(base))
        base = QDir::homePath() + QStringLiteral("/.config");
    return base + QStringLiteral("/nashi/inputd.toml");
}

QMap<QString, QString> InputdConfig::parseChords(const QString &text)
{
    QMap<QString, QString> chords{{QStringLiteral("shift"), QStringLiteral("convert")},
                                  {QStringLiteral("ctrl"), QStringLiteral("tools")},
                                  {QStringLiteral("alt"), QStringLiteral("actions")},
                                  {QStringLiteral("meta"), QString()}};
    const QStringList keys = modifierKeys();
    QString section;
    const auto lines = text.split(QLatin1Char('\n'));
    for (const QString &raw : lines) {
        const QString code = stripComment(raw);
        const QString sec = sectionOf(code);
        if (!sec.isEmpty()) {
            section = sec;
            continue;
        }
        const auto [key, value] = keyValueOf(code);
        if (key.isEmpty())
            continue;
        if (section == QLatin1String("chords") && keys.contains(key)) {
            chords[key] = normaliseMode(unquote(value));
        } else if (section.isEmpty() && key.startsWith(QLatin1String("chords."))
                   && keys.contains(key.mid(7))) {
            chords[key.mid(7)] = normaliseMode(unquote(value));
        } else if (section.isEmpty() && key == QLatin1String("chords") && value.startsWith(QLatin1Char('{'))) {
            static const QRegularExpression pair(QStringLiteral("([A-Za-z]+)\\s*=\\s*(\"[^\"]*\"|'[^']*')"));
            for (auto it = pair.globalMatch(value); it.hasNext();) {
                const auto m = it.next();
                if (keys.contains(m.captured(1)))
                    chords[m.captured(1)] = normaliseMode(unquote(m.captured(2)));
            }
        }
    }
    return chords;
}

QString InputdConfig::rewriteChords(const QString &text, const QMap<QString, QString> &chords)
{
    QStringList block;
    for (const QString &k : modifierKeys()) {
        const QString mode = normaliseMode(chords.value(k));
        block << QStringLiteral("%1 = \"%2\"").arg(k, mode.isEmpty() ? QStringLiteral("none") : mode);
    }

    const QStringList keys = modifierKeys();
    QStringList out;
    QString section;
    bool wrote = false;
    QStringList lines = text.split(QLatin1Char('\n'));
    if (!lines.isEmpty() && lines.constLast().isEmpty())
        lines.removeLast(); // trailing newline
    for (const QString &raw : std::as_const(lines)) {
        const QString code = stripComment(raw);
        const QString sec = sectionOf(code);
        if (!sec.isEmpty()) {
            section = sec;
            out << raw;
            if (sec == QLatin1String("chords") && !wrote) {
                out << block;
                wrote = true;
            }
            continue;
        }
        const auto [key, value] = keyValueOf(code);
        const bool drop = !key.isEmpty()
                          && ((section == QLatin1String("chords") && keys.contains(key))
                              || (section.isEmpty() && key.startsWith(QLatin1String("chords.")))
                              || (section.isEmpty() && key == QLatin1String("chords")));
        if (!drop)
            out << raw;
    }
    if (!wrote) {
        if (!out.isEmpty() && !out.constLast().trimmed().isEmpty())
            out << QString();
        out << QStringLiteral("[chords]") << block;
    }
    return out.join(QLatin1Char('\n')) + QLatin1Char('\n');
}

bool InputdConfig::writeChords(const QString &path, const QMap<QString, QString> &chords, QString *error)
{
    QString text;
    QFile in(path);
    if (in.exists()) {
        if (!in.open(QIODevice::ReadOnly)) {
            if (error)
                *error = QStringLiteral("cannot read %1: %2").arg(path, in.errorString());
            return false;
        }
        text = QString::fromUtf8(in.readAll());
        in.close();
    }
    const QString dir = QFileInfo(path).absolutePath();
    if (!QDir().mkpath(dir)) {
        if (error)
            *error = QStringLiteral("cannot create %1").arg(dir);
        return false;
    }
    // QSaveFile: temp file in the same directory, renamed over the target.
    QSaveFile out(path);
    if (!out.open(QIODevice::WriteOnly) || out.write(rewriteChords(text, chords).toUtf8()) < 0 || !out.commit()) {
        if (error)
            *error = QStringLiteral("cannot write %1: %2").arg(path, out.errorString());
        return false;
    }
    return true;
}

InputdClient::InputdClient(QObject *parent)
    : InputdClient(InputdConfig::defaultPath(), parent)
{
}

InputdClient::InputdClient(const QString &configPath, QObject *parent)
    : QObject(parent)
    , m_configPath(configPath)
{
    reloadChords();
    connectBus();
}

void InputdClient::connectBus()
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected())
        return;

    // Matches on the well-known name keep working across daemon restarts.
    bus.connect(kService, kPath, kInterface, QStringLiteral("DragStarted"), this, SLOT(onDragStarted()));
    bus.connect(kService, kPath, kInterface, QStringLiteral("ChordStarted"), this, SLOT(onChordStarted(QString)));
    bus.connect(kService, kPath, kInterface, QStringLiteral("ChordReleased"), this, SLOT(onChordReleased()));
    bus.connect(kService, kPath, kInterface, QStringLiteral("DragEnded"), this, SLOT(onDragEnded()));
    bus.connect(kService, kPath, kProps, QStringLiteral("PropertiesChanged"), this,
                SLOT(onPropertiesChanged(QString, QVariantMap, QStringList)));

    m_watcher = new QDBusServiceWatcher(kService, bus,
                                        QDBusServiceWatcher::WatchForRegistration | QDBusServiceWatcher::WatchForUnregistration,
                                        this);
    connect(m_watcher, &QDBusServiceWatcher::serviceRegistered, this, [this] {
        qCInfo(lcInputd) << "nashi-inputd appeared";
        setAvailable(true);
        fetchProperties();
    });
    connect(m_watcher, &QDBusServiceWatcher::serviceUnregistered, this, [this] {
        qCInfo(lcInputd) << "nashi-inputd went away";
        setAvailable(false);
        if (m_dragging || !m_mode.isEmpty()) {
            m_dragging = false;
            m_mode.clear();
            Q_EMIT stateChanged();
        }
        Q_EMIT daemonLost();
    });
    refresh();
}

void InputdClient::refresh()
{
    reloadChords();
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected() || !bus.interface())
        return;
    auto *w = new QDBusPendingCallWatcher(bus.interface()->asyncCall(QStringLiteral("NameHasOwner"), kService), this);
    connect(w, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *watcher) {
        const QDBusPendingReply<bool> reply = *watcher;
        const bool has = !reply.isError() && reply.value();
        setAvailable(has);
        if (has)
            fetchProperties();
        watcher->deleteLater();
    });
}

void InputdClient::setEnabled(bool on)
{
    if (!m_available)
        return;
    QDBusMessage msg = QDBusMessage::createMethodCall(kService, kPath, kProps, QStringLiteral("Set"));
    msg << kInterface << QStringLiteral("Enabled") << QVariant::fromValue(QDBusVariant(on));
    auto *w = new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(msg), this);
    connect(w, &QDBusPendingCallWatcher::finished, this, [this, on](QDBusPendingCallWatcher *watcher) {
        if (watcher->isError()) {
            qCWarning(lcInputd) << "Setting Enabled failed:" << watcher->error().message();
        } else if (m_enabled != on) {
            m_enabled = on;
            Q_EMIT enabledChanged();
        }
        watcher->deleteLater();
    });
}

void InputdClient::setAvailable(bool on)
{
    if (on == m_available)
        return;
    m_available = on;
    Q_EMIT availableChanged();
}

void InputdClient::fetchProperties()
{
    QDBusMessage msg = QDBusMessage::createMethodCall(kService, kPath, kProps, QStringLiteral("GetAll"));
    msg << kInterface;
    auto *w = new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(msg), this);
    connect(w, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *watcher) {
        const QDBusPendingReply<QVariantMap> reply = *watcher;
        if (!reply.isError())
            onPropertiesChanged(kInterface, reply.value(), {});
        watcher->deleteLater();
    });
}

void InputdClient::onPropertiesChanged(const QString &iface, const QVariantMap &changed, const QStringList &)
{
    if (iface != kInterface)
        return;
    if (changed.contains(QStringLiteral("Enabled"))) {
        const bool on = changed.value(QStringLiteral("Enabled")).toBool();
        if (on != m_enabled) {
            m_enabled = on;
            Q_EMIT enabledChanged();
        }
    }
    if (changed.contains(QStringLiteral("Version"))) {
        m_version = changed.value(QStringLiteral("Version")).toString();
        Q_EMIT availableChanged();
    }
}

void InputdClient::onDragStarted()
{
    m_dragging = true;
    Q_EMIT stateChanged();
    Q_EMIT dragStarted();
}

void InputdClient::onChordStarted(const QString &mode)
{
    m_dragging = true;
    m_mode = mode;
    Q_EMIT stateChanged();
    Q_EMIT chordStarted(mode);
}

void InputdClient::onChordReleased()
{
    m_mode.clear();
    Q_EMIT stateChanged();
    Q_EMIT chordReleased();
}

void InputdClient::onDragEnded()
{
    m_dragging = false;
    m_mode.clear();
    Q_EMIT stateChanged();
    Q_EMIT dragEnded();
}

QString InputdClient::keyFor(const QString &mode) const
{
    for (const QString &k : InputdConfig::modifierKeys()) {
        if (m_chords.value(k) == mode)
            return k;
    }
    return {};
}

void InputdClient::setConfigError(const QString &error)
{
    if (error == m_configError)
        return;
    m_configError = error;
    Q_EMIT configErrorChanged();
}

void InputdClient::reloadChords()
{
    QString text;
    QFile f(m_configPath);
    if (f.open(QIODevice::ReadOnly))
        text = QString::fromUtf8(f.readAll());
    const auto chords = InputdConfig::parseChords(text);
    if (chords != m_chords) {
        m_chords = chords;
        Q_EMIT chordsChanged();
    }
}

bool InputdClient::setChords(const QString &convertKey, const QString &toolsKey, const QString &actionsKey)
{
    const QStringList keys = InputdConfig::modifierKeys();
    const QStringList chosen{convertKey, toolsKey, actionsKey};
    for (const QString &k : chosen) {
        if (!keys.contains(k) || chosen.count(k) != 1) {
            setConfigError(tr("Each wheel needs its own key."));
            return false;
        }
    }
    QMap<QString, QString> chords;
    for (const QString &k : keys)
        chords.insert(k, QString());
    chords[convertKey] = QStringLiteral("convert");
    chords[toolsKey] = QStringLiteral("tools");
    chords[actionsKey] = QStringLiteral("actions");

    QString error;
    if (!InputdConfig::writeChords(m_configPath, chords, &error)) {
        qCWarning(lcInputd) << "Writing the chord config failed:" << error;
        setConfigError(tr("Could not save the drag chords: %1").arg(error));
        return false;
    }
    setConfigError(QString());
    reloadChords();
    reload();
    return true;
}

void InputdClient::reload()
{
    if (!m_available)
        return;
    const QDBusMessage msg = QDBusMessage::createMethodCall(kService, kPath, kInterface, QStringLiteral("Reload"));
    auto *w = new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(msg), this);
    connect(w, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *watcher) {
        if (watcher->isError()) {
            qCWarning(lcInputd) << "Reload failed:" << watcher->error().message();
            setConfigError(tr("nashi-inputd rejected its config: %1").arg(watcher->error().message()));
        } else {
            setConfigError(QString());
        }
        watcher->deleteLater();
    });
}
