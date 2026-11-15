#pragma once
#include "base/basic_types.h"
#include "base/flags.h"
#include "base/variant.h"
#include "base/flat_map.h"
#include "base/flat_set.h"
#include "mtproto/core_types.h"
#include "scheme.h"
#include "data/data_peer_id.h"
#include "data/data_msg_id.h"
#include "rpl/rpl.h"
#include <range/v3/all.hpp>
#include <QtCore/QCoreApplication>
#include <QtCore/QDateTime>
#include <iostream>
class PeerData;
class DocumentData;
class PhotoData;
using PhotoId = uint64;
using DocumentId = uint64;
struct GeoPointLocation;
