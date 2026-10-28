#include "wheelmodel.h"

#include <QJsonArray>

WheelModel::WheelModel(QObject *parent)
    : QAbstractListModel(parent)
{
}

QString WheelModel::normalizeMode(const QString &mode)
{
    const QString m = mode.trimmed().toLower();
    if (m == QLatin1String("tools") || m == QLatin1String("actions"))
        return m;
    return QStringLiteral("convert");
}

QString WheelModel::localizedTitle(const QJsonValue &title, const QString &language)
{
    if (title.isString())
        return title.toString();
    const QJsonObject obj = title.toObject();
    const QString wanted = obj.value(language).toString();
    if (!wanted.isEmpty())
        return wanted;
    const QString en = obj.value(QStringLiteral("en")).toString();
    if (!en.isEmpty())
        return en;
    return obj.isEmpty() ? QString() : obj.begin().value().toString();
}

QList<WheelItem> WheelModel::buildItems(const QJsonObject &wheel, const QString &mode, const QString &language)
{
    const QString m = normalizeMode(mode);
    const QString kind = m == QLatin1String("convert") ? QStringLiteral("convert")
                         : m == QLatin1String("tools") ? QStringLiteral("tool")
                                                        : QStringLiteral("action");
    QList<WheelItem> items;
    const QJsonArray list = wheel.value(m).toArray();
    for (const QJsonValue &v : list) {
        const QJsonObject o = v.toObject();
        WheelItem item;
        item.id = o.value(QStringLiteral("id")).toString();
        if (item.id.isEmpty())
            continue;
        if (o.value(QStringLiteral("needs_editor")).toBool())
            continue; // editors do not exist yet (docs/CLI-PROTOCOL.md)
        item.kind = kind;
        item.title = localizedTitle(o.value(QStringLiteral("title")), language);
        if (item.title.isEmpty())
            item.title = kind == QLatin1String("convert") ? item.id.toUpper() : item.id;
        item.icon = o.value(QStringLiteral("icon")).toString();
        item.available = o.value(QStringLiteral("available")).toBool(true);
        for (const QJsonValue &mv : o.value(QStringLiteral("missing")).toArray())
            item.missing.append(mv.toString());
        items.append(item);
    }
    return items;
}

int WheelModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_items.size());
}

QVariant WheelModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_items.size())
        return {};
    const WheelItem &it = m_items.at(index.row());
    switch (role) {
    case IdRole:
        return it.id;
    case KindRole:
        return it.kind;
    case Qt::DisplayRole:
    case TitleRole:
        return it.title;
    case IconRole:
        return it.icon;
    case AvailableRole:
        return it.available;
    case MissingRole:
        return it.missing.join(QStringLiteral(", "));
    default:
        return {};
    }
}

QHash<int, QByteArray> WheelModel::roleNames() const
{
    return {{IdRole, "itemId"},       {KindRole, "kind"},           {TitleRole, "title"},
            {IconRole, "iconName"},   {AvailableRole, "available"}, {MissingRole, "missing"}};
}

void WheelModel::setMode(const QString &mode)
{
    const QString m = normalizeMode(mode);
    if (m == m_mode)
        return;
    m_mode = m;
    Q_EMIT modeChanged();
    rebuild();
}

void WheelModel::setLanguage(const QString &language)
{
    if (language == m_language)
        return;
    m_language = language;
    Q_EMIT languageChanged();
    rebuild();
}

void WheelModel::setWheel(const QJsonObject &wheel)
{
    m_wheel = wheel;
    m_hasData = true;
    rebuild();
}

void WheelModel::clear()
{
    m_wheel = {};
    m_hasData = false;
    rebuild();
}

QVariantMap WheelModel::get(int row) const
{
    if (row < 0 || row >= m_items.size())
        return {};
    const WheelItem &it = m_items.at(row);
    return {{QStringLiteral("itemId"), it.id},
            {QStringLiteral("kind"), it.kind},
            {QStringLiteral("title"), it.title},
            {QStringLiteral("iconName"), it.icon},
            {QStringLiteral("available"), it.available},
            {QStringLiteral("missing"), it.missing.join(QStringLiteral(", "))}};
}

int WheelModel::countFor(const QString &mode) const
{
    return int(buildItems(m_wheel, mode, m_language).size());
}

void WheelModel::rebuild()
{
    beginResetModel();
    m_items = buildItems(m_wheel, m_mode, m_language);
    endResetModel();
    Q_EMIT countChanged();
}
