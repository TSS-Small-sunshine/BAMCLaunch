import {
  Alert,
  AlertIcon,
  Badge,
  Box,
  Button,
  Divider,
  Flex,
  Modal,
  ModalBody,
  ModalCloseButton,
  ModalContent,
  ModalFooter,
  ModalHeader,
  ModalOverlay,
  Skeleton,
  Text,
  Tooltip,
  useDisclosure,
  VStack,
} from '@chakra-ui/react';
import { DownloadIcon, RepeatIcon } from '@chakra-ui/icons';
import type { ManifestVersion, LatestVersions } from '../types/version';
import { useVersionManifest } from '../hooks/useVersionManifest';
import { useVersionDownload } from '../hooks/useVersionDownload';
import { useVersionJar } from '../hooks/useVersionJar';
import { useVersionAssets } from '../hooks/useVersionAssets';
import { useVersionLibraries } from '../hooks/useVersionLibraries';
import { useVersionJava } from '../hooks/useVersionJava';
import { useVersionLaunch } from '../hooks/useVersionLaunch';
import type { JavaCandidate, JavaScanResult } from '../lib/tauri';

/** 把版本按 正式版/快照版 分组,并按发布时间倒序 */
function groupVersions(versions: ManifestVersion[]) {
  const release = versions
    .filter((v) => v.type === 'release')
    .sort((a, b) => b.releaseTime.localeCompare(a.releaseTime));
  const snapshot = versions
    .filter((v) => v.type === 'snapshot')
    .sort((a, b) => b.releaseTime.localeCompare(a.releaseTime));
  return { release, snapshot };
}

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  });
}

/** 单个版本卡片 */
function VersionCard({ version, isLatest }: { version: ManifestVersion; isLatest: boolean }) {
  const isRelease = version.type === 'release';
  const { state, download } = useVersionDownload(version.id, version.url);
  const jar = useVersionJar(version.id);
  const assets = useVersionAssets(version.id);
  const libraries = useVersionLibraries(version.id);
  const java = useVersionJava(version.id);
  const launch = useVersionLaunch(version.id);
  const javaModal = useDisclosure();
  return (
    <Flex
      align="center"
      gap={4}
      bg="white"
      borderRadius="card"
      border="1px solid"
      borderColor="brand.100"
      boxShadow="card"
      px={4}
      py={3.5}
      _hover={{ boxShadow: 'cardHover', transform: 'translateY(-1px)' }}
      transition="all 0.15s"
    >
      {/* MC 像素块图标 */}
      <Box
        w={9}
        h={9}
        flexShrink={0}
        borderRadius="lg"
        bg={isRelease ? 'grass.100' : 'brand.100'}
        display="flex"
        alignItems="center"
        justifyContent="center"
      >
        <Box
          w="14px"
          h="14px"
          borderRadius="3px"
          bg={isRelease ? 'grass.500' : 'brand.400'}
          boxShadow="0 0 0 3px rgba(255,255,255,0.6)"
        />
      </Box>

      <Box flex={1} minW={0}>
        <Flex align="center" gap={2}>
          <Text fontWeight="800" fontSize="md" color="gray.800" noOfLines={1}>
            {version.id}
          </Text>
          <Badge colorScheme={isRelease ? 'grass' : 'brand'}>
            {isRelease ? '正式版' : '快照版'}
          </Badge>
          {isLatest && (
            <Badge colorScheme="yellow" variant="solid">
              最新
            </Badge>
          )}
        </Flex>
        <Text fontSize="xs" color="gray.400" mt={0.5}>
          发布于 {formatDate(version.releaseTime)}
        </Text>
      </Box>

      {state.status === 'done' ? (
        <Tooltip label={`已保存: ${state.path}`} placement="top">
          <Button
            size="sm"
            leftIcon={<DownloadIcon />}
            onClick={download}
            colorScheme="grass"
            variant="outline"
          >
            已下载
          </Button>
        </Tooltip>
      ) : (
        <Button
          size="sm"
          leftIcon={<DownloadIcon />}
          onClick={download}
          isLoading={state.status === 'downloading'}
        >
          {state.status === 'idle' && '下载'}
          {state.status === 'downloading' && '下载中'}
          {state.status === 'error' && '重试'}
        </Button>
      )}
      {jar.state.status === 'done' ? (
        <Tooltip label={`已保存: ${jar.state.path}`} placement="top">
          <Button size="sm" colorScheme="grass" variant="outline" onClick={jar.download}>
            客户端
          </Button>
        </Tooltip>
      ) : (
        <Tooltip
          label={jar.state.status === 'error' ? jar.state.message : '下载客户端 jar'}
          placement="top"
        >
          <Button size="sm" onClick={jar.download} isLoading={jar.state.status === 'downloading'}>
            {jar.state.status === 'idle' && '客户端'}
            {jar.state.status === 'downloading' && '下载中'}
            {jar.state.status === 'error' && '重试'}
          </Button>
        </Tooltip>
      )}
      {assets.state.status === 'done' ? (
        <Tooltip
          label={`资源完成: 新增 ${assets.state.summary.downloaded} / 共 ${assets.state.summary.total}, 跳过 ${assets.state.summary.skipped}`}
          placement="top"
        >
          <Button size="sm" colorScheme="grass" variant="outline" onClick={assets.download}>
            资源
          </Button>
        </Tooltip>
      ) : (
        <Tooltip
          label={
            assets.state.status === 'error'
              ? assets.state.message
              : '下载全部资源(音效/语言/字体等, 首次较大)'
          }
          placement="top"
        >
          <Button
            size="sm"
            onClick={assets.download}
            isLoading={assets.state.status === 'downloading'}
          >
            {assets.state.status === 'idle' && '资源'}
            {assets.state.status === 'downloading' && '下载中'}
            {assets.state.status === 'error' && '重试'}
          </Button>
        </Tooltip>
      )}
      {libraries.state.status === 'done' ? (
        <Tooltip
          label={`库完成: 新增 ${libraries.state.summary.downloaded} / 共 ${libraries.state.summary.total}, 跳过 ${libraries.state.summary.skipped}, 原生库 ${libraries.state.summary.natives}`}
          placement="top"
        >
          <Button size="sm" colorScheme="grass" variant="outline" onClick={libraries.download}>
            库
          </Button>
        </Tooltip>
      ) : (
        <Tooltip
          label={
            libraries.state.status === 'error'
              ? libraries.state.message
              : '下载运行库(含 Windows 原生库, 首次较大)'
          }
          placement="top"
        >
          <Button
            size="sm"
            onClick={libraries.download}
            isLoading={libraries.state.status === 'downloading'}
          >
            {libraries.state.status === 'idle' && '库'}
            {libraries.state.status === 'downloading' && '下载中'}
            {libraries.state.status === 'error' && '重试'}
          </Button>
        </Tooltip>
      )}
      <Tooltip
        label={
          java.state.status === 'error'
            ? java.state.message
            : java.state.status === 'done'
              ? `已发现 ${java.state.result.candidates.length} 个 Java;适配 Java ${java.state.result.required_major} 的 ${java.state.result.candidates.filter((c) => c.meets_requirement).length} 个`
              : '扫描本机 Java 安装并检查版本适配'
        }
        placement="top"
      >
        <Button
          size="sm"
          onClick={() => {
            javaModal.onOpen();
            if (java.state.status !== 'done') {
              void java.scan();
            }
          }}
          isLoading={java.state.status === 'scanning'}
          colorScheme={java.state.status === 'done' ? 'grass' : undefined}
          variant={java.state.status === 'done' ? 'outline' : undefined}
        >
          {java.state.status === 'idle' && 'Java'}
          {java.state.status === 'scanning' && '扫描中'}
          {java.state.status === 'done' && 'Java'}
          {java.state.status === 'error' && '重试'}
        </Button>
      </Tooltip>
      <Tooltip
        label={
          launch.state.status === 'launched'
            ? `已启动 (pid ${launch.state.result.pid})`
            : launch.state.status === 'error'
              ? launch.state.message
              : '选择 Java 并启动游戏(离线模式)'
        }
        placement="top"
      >
        <Button
          size="sm"
          onClick={() => {
            javaModal.onOpen();
            if (java.state.status !== 'done') {
              void java.scan();
            }
          }}
          colorScheme={launch.state.status === 'launched' ? 'grass' : undefined}
          variant={launch.state.status === 'launched' ? 'outline' : undefined}
        >
          {launch.state.status === 'idle' && '启动'}
          {launch.state.status === 'launching' && '启动中'}
          {launch.state.status === 'launched' && '已启动'}
          {launch.state.status === 'error' && '重试'}
        </Button>
      </Tooltip>
      <JavaCandidatesModal
        isOpen={javaModal.isOpen}
        onClose={() => {
          javaModal.onClose();
          java.reset();
        }}
        scanState={java.state}
        versionId={version.id}
        onRetry={java.scan}
        onLaunch={(javaPath) => void launch.launch(javaPath)}
        launching={launch.state.status === 'launching'}
        launched={launch.state.status === 'launched' ? launch.state.result : null}
      />
    </Flex>
  );
}

/** L5:Java 候选列表 Modal —— 按适配/不适配分组,源标 JAVA_HOME/PATH/CommonDir/Registry */
function JavaCandidatesModal({
  isOpen,
  onClose,
  scanState,
  versionId,
  onRetry,
  onLaunch,
  launching,
  launched,
}: {
  isOpen: boolean;
  onClose: () => void;
  scanState: ReturnType<typeof useVersionJava>['state'];
  versionId: string;
  onRetry: () => Promise<void>;
  onLaunch: (javaPath: string) => void;
  launching: boolean;
  launched: { pid: number; java_path: string } | null;
}) {
  return (
    <Modal isOpen={isOpen} onClose={onClose} size="lg" isCentered>
      <ModalOverlay />
      <ModalContent borderRadius="card">
        <ModalHeader>Java 候选 · {versionId}</ModalHeader>
        <ModalCloseButton />
        <ModalBody pb={4}>
          {scanState.status === 'scanning' && (
            <Text color="gray.500" fontSize="sm">
              正在扫描本机 Java 安装...
            </Text>
          )}
          {scanState.status === 'error' && (
            <Alert status="error" borderRadius="card" bg="red.50">
              <AlertIcon />
              <Box>
                <Text fontWeight="700" color="red.600" fontSize="sm">
                  扫描失败
                </Text>
                <Text fontSize="xs" color="red.500" mt={1}>
                  {scanState.message}
                </Text>
              </Box>
            </Alert>
          )}
          {scanState.status === 'done' && (
            <JavaScanResultBody
              result={scanState.result}
              onLaunch={onLaunch}
              launching={launching}
              launched={launched}
            />
          )}
        </ModalBody>
        <ModalFooter gap={2}>
          {scanState.status === 'done' && (
            <Button size="sm" variant="ghost" onClick={() => void onRetry()}>
              重新扫描
            </Button>
          )}
          <Button size="sm" colorScheme="brand" onClick={onClose}>
            关闭
          </Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}

/** L5:Modal 内文 —— 分适配/不适配两组 + 每行启动按钮(L6) */
function JavaScanResultBody({
  result,
  onLaunch,
  launching,
  launched,
}: {
  result: JavaScanResult;
  onLaunch: (javaPath: string) => void;
  launching: boolean;
  launched: { pid: number; java_path: string } | null;
}) {
  if (result.candidates.length === 0) {
    return (
      <Alert status="info" borderRadius="card" bg="blue.50">
        <AlertIcon />
        <Box>
          <Text fontWeight="700" color="blue.700" fontSize="sm">
            未检测到 Java 安装
          </Text>
          <Text fontSize="xs" color="blue.600" mt={1}>
            请安装 Java {result.required_major}+ 或在设置中手动指定路径(M3 实装)
          </Text>
        </Box>
      </Alert>
    );
  }
  const meeting = result.candidates.filter((c) => c.meets_requirement);
  const notMeeting = result.candidates.filter((c) => !c.meets_requirement);
  return (
    <VStack align="stretch" spacing={4}>
      <Text fontSize="sm" color="gray.600">
        此版本需要 <b>Java {result.required_major}+</b>,已扫描到 {result.candidates.length} 个 Java
        实例
        {launched && <> · 已启动 (pid {launched.pid})</>}
      </Text>
      {meeting.length > 0 && (
        <Box>
          <Text fontSize="xs" fontWeight="700" color="grass.600" mb={2}>
            ✓ 满足版本要求({meeting.length})
          </Text>
          <VStack align="stretch" spacing={2}>
            {meeting.map((c, i) => (
              <JavaCandidateRow
                key={`m-${i}`}
                candidate={c}
                onLaunch={onLaunch}
                launching={launching}
              />
            ))}
          </VStack>
        </Box>
      )}
      {notMeeting.length > 0 && (
        <Box>
          <Divider my={1} />
          <Text fontSize="xs" fontWeight="700" color="gray.500" mb={2} mt={3}>
            ✗ 版本过低或不可用({notMeeting.length})
          </Text>
          <VStack align="stretch" spacing={2}>
            {notMeeting.map((c, i) => (
              <JavaCandidateRow
                key={`n-${i}`}
                candidate={c}
                onLaunch={onLaunch}
                launching={launching}
                disabled
              />
            ))}
          </VStack>
        </Box>
      )}
    </VStack>
  );
}

/** L5+6:单个 Java 候选行 —— 版本号 + 来源标签 + 路径 + 启动按钮 */
function JavaCandidateRow({
  candidate,
  onLaunch,
  launching,
  disabled,
}: {
  candidate: JavaCandidate;
  onLaunch: (javaPath: string) => void;
  launching: boolean;
  disabled?: boolean;
}) {
  return (
    <Flex align="center" gap={3} bg="gray.50" borderRadius="lg" px={3} py={2}>
      <Text
        fontWeight="800"
        fontSize="md"
        color={candidate.meets_requirement ? 'grass.600' : 'gray.500'}
        minW="40px"
      >
        v{candidate.version}
      </Text>
      <Badge colorScheme={sourceColor(candidate.source)} variant="subtle">
        {sourceLabel(candidate.source)}
      </Badge>
      <Text fontSize="xs" color="gray.500" noOfLines={1} flex={1} fontFamily="mono">
        {candidate.path}
      </Text>
      <Button
        size="xs"
        colorScheme="brand"
        variant={candidate.meets_requirement ? 'solid' : 'outline'}
        onClick={() => onLaunch(candidate.path)}
        isLoading={launching && candidate.meets_requirement}
        isDisabled={disabled || !candidate.meets_requirement}
      >
        启动
      </Button>
    </Flex>
  );
}

function sourceColor(source: JavaCandidate['source']): string {
  switch (source) {
    case 'java_home':
      return 'brand';
    case 'path':
      return 'purple';
    case 'common_dir':
      return 'orange';
    case 'registry':
      return 'gray';
  }
}

function sourceLabel(source: JavaCandidate['source']): string {
  switch (source) {
    case 'java_home':
      return 'JAVA_HOME';
    case 'path':
      return 'PATH';
    case 'common_dir':
      return 'CommonDir';
    case 'registry':
      return 'Registry';
  }
}

/** 版本分组区块 */
function VersionGroup({
  title,
  count,
  versions,
  latestId,
}: {
  title: string;
  count: number;
  versions: ManifestVersion[];
  latestId?: string;
}) {
  if (versions.length === 0) return null;
  return (
    <Box mb={8}>
      <Text fontWeight="800" fontSize="lg" color="gray.700" mb={3}>
        {title}
        <Box as="span" color="gray.400" fontSize="sm" fontWeight="600" ml={2}>
          {count} 个版本
        </Box>
      </Text>
      <VStack spacing={2.5} align="stretch">
        {versions.map((v) => (
          <VersionCard key={v.id} version={v} isLatest={v.id === latestId} />
        ))}
      </VStack>
    </Box>
  );
}

/** 加载中的骨架卡片 */
function LoadingSkeleton() {
  return (
    <VStack spacing={2.5} align="stretch">
      {Array.from({ length: 5 }, (_, i) => (
        <Flex key={i} align="center" gap={4} bg="white" borderRadius="card" p={4}>
          <Skeleton w={9} h={9} borderRadius="lg" />
          <Box flex={1}>
            <Skeleton h="16px" w="120px" mb={2} />
            <Skeleton h="12px" w="80px" />
          </Box>
          <Skeleton w="64px" h="32px" borderRadius="full" />
        </Flex>
      ))}
    </VStack>
  );
}

/** 主页:Minecraft 版本列表 */
export default function HomePage() {
  const result = useVersionManifest();
  const reload = result.reload;

  return (
    <Box maxW="880px" mx="auto">
      <Flex direction="column" gap={3} mb={7}>
        {/* MC 草方块像素条 */}
        <Flex gap="3px">
          {Array.from({ length: 22 }, (_, i) => (
            <Box
              key={i}
              w="7px"
              h="7px"
              borderRadius="2px"
              bg={i % 2 === 0 ? 'grass.400' : 'grass.600'}
            />
          ))}
        </Flex>

        <Flex align="baseline" justify="space-between">
          <Box>
            <Text fontSize="3xl" fontWeight="800" color="gray.800">
              欢迎回来
            </Text>
            <Text fontSize="sm" color="gray.500" mt={1}>
              从 Mojang 官方清单读取版本 · 选择一个版本开始冒险
            </Text>
          </Box>
          <Button
            size="sm"
            variant="ghost"
            leftIcon={<RepeatIcon />}
            onClick={reload}
            isLoading={result.status === 'loading'}
          >
            刷新
          </Button>
        </Flex>
      </Flex>

      {result.status === 'loading' && <LoadingSkeleton />}

      {result.status === 'error' && (
        <Alert status="error" borderRadius="card" bg="red.50" flexDirection="column" gap={3} py={5}>
          <AlertIcon />
          <Box textAlign="center">
            <Text fontWeight="700" color="red.600">
              版本清单获取失败
            </Text>
            <Text fontSize="sm" color="red.400" mt={1} noOfLines={3}>
              {result.message}
            </Text>
          </Box>
          <Button size="sm" colorScheme="red" variant="outline" onClick={reload}>
            重试
          </Button>
        </Alert>
      )}

      {result.status === 'success' && result.manifest && (
        <VersionGroups latest={result.manifest.latest} versions={result.manifest.versions} />
      )}
    </Box>
  );
}

function VersionGroups({
  latest,
  versions,
}: {
  latest: LatestVersions;
  versions: ManifestVersion[];
}) {
  const { release, snapshot } = groupVersions(versions);
  const isEmpty = release.length === 0 && snapshot.length === 0;

  if (isEmpty) {
    return (
      <Alert status="info" borderRadius="card">
        <AlertIcon />
        暂时没有版本数据
      </Alert>
    );
  }

  return (
    <>
      <VersionGroup
        title="正式版"
        count={release.length}
        versions={release}
        latestId={latest.release}
      />
      <VersionGroup
        title="快照版"
        count={snapshot.length}
        versions={snapshot}
        latestId={latest.snapshot}
      />
    </>
  );
}
