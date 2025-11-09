import { Variants, Transition } from 'framer-motion';

export const transition: Transition = {
  duration: 0.3,
  ease: 'easeInOut',
};

export const fadeInUp: Variants = {
  initial: { opacity: 0, y: 16, willChange: 'transform, opacity' },
  animate: { opacity: 1, y: 0, transition },
  exit: { opacity: 0, y: 16, transition },
};

export const slideInLeft: Variants = {
  initial: { opacity: 0, x: -16, willChange: 'transform, opacity' },
  animate: { opacity: 1, x: 0, transition },
  exit: { opacity: 0, x: -16, transition },
};

export const fadeScale: Variants = {
  initial: { opacity: 0, scale: 0.98, willChange: 'transform, opacity' },
  animate: { opacity: 1, scale: 1, transition },
  exit: { opacity: 0, scale: 0.98, transition },
};

export const staggerContainer = (stagger: number = 0.06): Variants => ({
  initial: {},
  animate: { transition: { staggerChildren: stagger } },
});

export const pageFade: Variants = {
  initial: { opacity: 0 },
  animate: { opacity: 1, transition: { duration: 0.25, ease: 'easeInOut' } },
  exit: { opacity: 0, transition: { duration: 0.2, ease: 'easeInOut' } },
};

export const glowPulse: Variants = {
  animate: {
    filter: ['drop-shadow(0 0 0px rgba(0,245,212,0.0))', 'drop-shadow(0 0 8px rgba(0,245,212,0.35))', 'drop-shadow(0 0 0px rgba(0,245,212,0.0))'],
    transition: { duration: 3, repeat: Infinity, ease: 'easeInOut' },
  },
};
