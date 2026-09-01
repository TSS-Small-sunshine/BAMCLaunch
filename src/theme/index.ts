import { extendTheme } from '@chakra-ui/react';

/**
 * BAMCLaunch 设计令牌 —— 蔚蓝档案(Blue Archive)× 我的世界(Minecraft)融合
 * - BA: 明亮浅蓝主色 #4C9EEB、白色卡片、大圆角、柔和光晕
 * - MC: 草方块绿 #7CBD4B 作为强调色、像素化细节点缀
 */
export const theme = extendTheme({
  colors: {
    // BA 蓝(Azur 蓝),50~900 色阶
    brand: {
      50: '#EAF4FF',
      100: '#D6E9FB',
      200: '#ABD2F6',
      300: '#80BAF0',
      400: '#5DA7EC',
      500: '#4C9EEB',
      600: '#3B84C9',
      700: '#2C669C',
      800: '#1D476E',
      900: '#0F2941',
    },
    // MC 草方块绿
    grass: {
      50: '#F2FAE9',
      100: '#E2F3CC',
      200: '#C8E8A3',
      300: '#A9DB73',
      400: '#92CA5B',
      500: '#7CBD4B',
      600: '#5D9A34',
      700: '#467826',
      800: '#30541B',
      900: '#1D3410',
    },
  },
  fonts: {
    heading: `"Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif`,
    body: `"Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif`,
  },
  radii: {
    card: '16px',
    pill: '999px',
  },
  shadows: {
    card: '0 4px 16px rgba(76, 158, 235, 0.14)',
    cardHover: '0 8px 24px rgba(76, 158, 235, 0.22)',
    glow: '0 4px 16px rgba(76, 158, 235, 0.45)',
  },
  components: {
    Button: {
      baseStyle: {
        fontWeight: '700',
        borderRadius: 'pill',
      },
      variants: {
        solid: {
          bg: 'brand.500',
          color: 'white',
          boxShadow: 'glow',
          _hover: { bg: 'brand.600' },
          _active: { bg: 'brand.700' },
        },
        ghost: {
          color: 'brand.600',
          _hover: { bg: 'brand.50' },
        },
      },
    },
    Card: {
      baseStyle: {
        bg: 'white',
        borderRadius: 'card',
        boxShadow: 'card',
        border: '1px solid',
        borderColor: 'brand.100',
      },
    },
    Badge: {
      baseStyle: {
        borderRadius: 'full',
        fontWeight: '700',
        px: 2.5,
        py: 0.5,
      },
    },
  },
});

export default theme;
